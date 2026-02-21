using System.Diagnostics;
using System.Text.RegularExpressions;
using System.Text;
using Windows.Foundation;
using Windows.Management.Deployment;

static class P
{
    static string? HelperLogPath;
    static readonly string[] MinecraftFamilies = new[]
    {
        "Microsoft.MinecraftUWP_8wekyb3d8bbwe",
        "Microsoft.MinecraftWindowsBeta_8wekyb3d8bbwe"
    };

    static int Main(string[] args)
    {
        try
        {
            HelperLogPath = Environment.GetEnvironmentVariable("BEDROCK_GDK_HELPER_LOG");
            Log($"START args={string.Join(" ", args)}");
            if (args.Length == 0 || args[0] != "install-gdk")
            {
                Console.Error.WriteLine("ERROR|Usage: install-gdk --msixvc <path> --output <dir>");
                Log("ERROR Usage: install-gdk --msixvc <path> --output <dir>");
                return 2;
            }

            string msixvc = "";
            string output = "";
            for (int i = 1; i < args.Length - 1; i++)
            {
                if (args[i] == "--msixvc") msixvc = args[++i];
                else if (args[i] == "--output") output = args[++i];
            }
            if (string.IsNullOrWhiteSpace(msixvc) || string.IsNullOrWhiteSpace(output))
            {
                Console.Error.WriteLine("ERROR|Missing --msixvc or --output");
                Log("ERROR Missing --msixvc or --output");
                return 2;
            }

            return Install(msixvc, output).GetAwaiter().GetResult();
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine("ERROR|" + ex);
            Log("FATAL " + ex);
            return 1;
        }
    }

    static void Progress(int p, string msg)
    {
        Console.WriteLine($"PROGRESS|{Math.Clamp(p, 0, 100)}|{msg}");
        Log($"PROGRESS {Math.Clamp(p, 0, 100)} {msg}");
    }

    static void Log(string message)
    {
        try
        {
            if (string.IsNullOrWhiteSpace(HelperLogPath))
                return;
            var line = $"[{DateTime.UtcNow:O}] {message}{Environment.NewLine}";
            File.AppendAllText(HelperLogPath!, line);
        }
        catch { }
    }

    static async Task<int> Install(string msixvcPath, string outputDir)
    {
        msixvcPath = Path.GetFullPath(msixvcPath);
        outputDir = Path.GetFullPath(outputDir);
        if (!File.Exists(msixvcPath))
        {
            Console.Error.WriteLine("ERROR|MSIXVC not found: " + msixvcPath);
            Log("ERROR MSIXVC not found: " + msixvcPath);
            return 1;
        }

        Directory.CreateDirectory(outputDir);
        var expectedVersionPrefix = ParseExpectedVersionPrefix(msixvcPath);
        Progress(2, "Clearing previous staged Minecraft");
        await ClearExistingStagedMinecraft(pm: new PackageManager());
        Progress(5, "Staging GDK package");

        var pm = new PackageManager();
        var uri = new Uri(msixvcPath);
        var stageOp = pm.StagePackageAsync(uri, null);
        var tcs = new TaskCompletionSource<int>();
        stageOp.Progress = (op, prog) =>
        {
            var pct = 5 + (int)Math.Round(prog.percentage * 0.35);
            Progress(pct, "Staging GDK package");
        };
        stageOp.Completed = (op, status) =>
        {
            try
            {
                if (status == AsyncStatus.Error)
                {
                    var res = op.GetResults();
                    tcs.TrySetException(new Exception($"Stage failed: {res.ErrorText} ({res.ExtendedErrorCode})"));
                }
                else
                {
                    tcs.TrySetResult(0);
                }
            }
            catch (Exception ex)
            {
                tcs.TrySetException(ex);
            }
        };
        await tcs.Task;

        var candidate = await FindStagedInstallLocationWithRetry(pm, TimeSpan.FromMinutes(3), expectedVersionPrefix);
        if (candidate.installPath is null || candidate.packageFamily is null)
        {
            Console.Error.WriteLine("ERROR|GDK package was staged, but staged Minecraft path was not found");
            return 1;
        }

        var src = candidate.installPath;
        var pfn = candidate.packageFamily;
        var exeSrc = Path.Combine(src, "Minecraft.Windows.exe");
        if (!File.Exists(exeSrc))
        {
            Console.Error.WriteLine("ERROR|Staged executable missing: " + exeSrc);
            return 1;
        }

        Progress(50, "Copying staged GDK files");
        if (Directory.Exists(outputDir))
        {
            try { Directory.Delete(outputDir, true); } catch { }
        }
        var moved = false;
        try
        {
            var srcRoot = Path.GetPathRoot(src);
            var dstRoot = Path.GetPathRoot(outputDir);
            if (!string.IsNullOrWhiteSpace(srcRoot)
                && !string.IsNullOrWhiteSpace(dstRoot)
                && string.Equals(srcRoot, dstRoot, StringComparison.OrdinalIgnoreCase))
            {
                Directory.Move(src, outputDir);
                moved = true;
            }
        }
        catch
        {
            moved = false;
        }
        if (!moved)
        {
            Directory.CreateDirectory(outputDir);
            CopyDirectorySkipExe(src, outputDir);
        }

        Progress(72, "Copying decrypted Minecraft.Windows.exe");
        var exeDst = Path.Combine(outputDir, "Minecraft.Windows.exe");
        await CopyExeWithFallbacks(pfn, exeSrc, exeDst);

        // mc-w10-style flow: do not register local extracted package.
        // Registering may bind it back to Store update behavior.
        Progress(88, "Finalizing local GDK files");

        if (!File.Exists(exeDst))
        {
            Console.Error.WriteLine("ERROR|Local GDK executable missing after extract");
            return 1;
        }

        Progress(100, "GDK package ready");
        Console.WriteLine($"RESULT|{outputDir}|{pfn}");
        return 0;
    }

    static string? ParseExpectedVersionPrefix(string msixvcPath)
    {
        var fileName = Path.GetFileName(msixvcPath);
        if (string.IsNullOrWhiteSpace(fileName)) return null;
        var m = Regex.Match(fileName, @"(\d+\.\d+\.\d+(?:\.\d+)?)");
        if (!m.Success) return null;
        return m.Groups[1].Value;
    }

    static string PackageVersionString(Windows.ApplicationModel.PackageVersion v)
    {
        return $"{v.Major}.{v.Minor}.{v.Build}.{v.Revision}";
    }

    static bool PackageVersionMatchesPrefix(Windows.ApplicationModel.PackageVersion v, string? expectedPrefix)
    {
        if (string.IsNullOrWhiteSpace(expectedPrefix)) return true;
        var s = PackageVersionString(v);
        return s.StartsWith(expectedPrefix, StringComparison.OrdinalIgnoreCase);
    }

    static async Task ClearExistingStagedMinecraft(PackageManager pm)
    {
        foreach (var family in MinecraftFamilies)
        {
            var toRemove = pm.FindPackages(family).ToList();
            foreach (var pkg in toRemove)
            {
                try
                {
                    var fullName = pkg.Id.FullName;
                    if (!string.IsNullOrWhiteSpace(fullName))
                    {
                        await pm.RemovePackageAsync(fullName).AsTask();
                    }
                }
                catch { }
            }
        }
    }

    static async Task<(string? installPath, string? packageFamily)> FindStagedInstallLocationWithRetry(PackageManager pm, TimeSpan timeout, string? expectedVersionPrefix)
    {
        var sw = Stopwatch.StartNew();
        while (sw.Elapsed < timeout)
        {
            var candidate = FindStagedInstallLocation(pm, expectedVersionPrefix);
            if (candidate.installPath is not null && candidate.packageFamily is not null)
            {
                return candidate;
            }
            await Task.Delay(1500);
        }
        return (null, null);
    }

    static (string? installPath, string? packageFamily) FindStagedInstallLocation(PackageManager pm, string? expectedVersionPrefix)
    {
        var familyMatches = new List<(string path, string family, Windows.ApplicationModel.PackageVersion version)>();

        foreach (var family in MinecraftFamilies)
        {
            foreach (var pkg in pm.FindPackages(family))
            {
                try
                {
                    var path = pkg.InstalledLocation?.Path;
                    if (string.IsNullOrWhiteSpace(path)) continue;
                    var resolved = ResolveMaybeLink(path);
                    if (File.Exists(Path.Combine(resolved, "Minecraft.Windows.exe")))
                    {
                        familyMatches.Add((resolved, family, pkg.Id.Version));
                    }
                    else if (File.Exists(Path.Combine(path, "Minecraft.Windows.exe")))
                    {
                        familyMatches.Add((path, family, pkg.Id.Version));
                    }
                }
                catch { }
            }
        }

        if (familyMatches.Count > 0)
        {
            var exact = familyMatches
                .Where(x => PackageVersionMatchesPrefix(x.version, expectedVersionPrefix))
                .OrderByDescending(x => x.version.Major)
                .ThenByDescending(x => x.version.Minor)
                .ThenByDescending(x => x.version.Build)
                .ThenByDescending(x => x.version.Revision)
                .FirstOrDefault();
            if (exact.path is not null) return (exact.path, exact.family);

            var fallback = familyMatches
                .OrderByDescending(x => x.version.Major)
                .ThenByDescending(x => x.version.Minor)
                .ThenByDescending(x => x.version.Build)
                .ThenByDescending(x => x.version.Revision)
                .First();
            return (fallback.path, fallback.family);
        }

        var roots = new[] {
            @"C:\XboxGames\Minecraft for Windows\Content",
            Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles), "ModifiableWindowsApps", "Minecraft for Windows", "Content")
        };

        foreach (var root in roots)
        {
            if (File.Exists(Path.Combine(root, "Minecraft.Windows.exe")))
            {
                var pfn = FindMatchingPackageFamily(pm, root);
                return (root, pfn ?? "Microsoft.MinecraftUWP_8wekyb3d8bbwe");
            }
        }

        foreach (var pkg in pm.FindPackages())
        {
            try
            {
                var id = pkg.Id;
                var fam = id.FamilyName;
                if (!fam.StartsWith("Microsoft.Minecraft", StringComparison.OrdinalIgnoreCase)) continue;
                var path = pkg.InstalledLocation?.Path;
                if (string.IsNullOrWhiteSpace(path)) continue;
                var resolved = ResolveMaybeLink(path);
                if (File.Exists(Path.Combine(resolved, "Minecraft.Windows.exe"))) return (resolved, fam);
                if (File.Exists(Path.Combine(path, "Minecraft.Windows.exe"))) return (path, fam);
            }
            catch { }
        }

        return (null, null);
    }

    static string ResolveMaybeLink(string path)
    {
        try
        {
            var di = new DirectoryInfo(path);
            var target = di.ResolveLinkTarget(true);
            if (target is not null && target.Exists)
            {
                return target.FullName;
            }
        }
        catch { }
        return path;
    }

    static string? FindMatchingPackageFamily(PackageManager pm, string location)
    {
        foreach (var pkg in pm.FindPackages())
        {
            try
            {
                var fam = pkg.Id.FamilyName;
                if (!fam.StartsWith("Microsoft.Minecraft", StringComparison.OrdinalIgnoreCase)) continue;
                var path = pkg.InstalledLocation?.Path;
                if (string.IsNullOrWhiteSpace(path)) continue;
                if (string.Equals(Path.GetFullPath(path), Path.GetFullPath(location), StringComparison.OrdinalIgnoreCase))
                {
                    return fam;
                }
            }
            catch { }
        }
        return null;
    }

    static void CopyDirectorySkipExe(string sourceDir, string destinationDir)
    {
        Directory.CreateDirectory(destinationDir);
        foreach (var file in Directory.EnumerateFiles(sourceDir, "*", SearchOption.AllDirectories))
        {
            var rel = Path.GetRelativePath(sourceDir, file);
            if (string.Equals(rel, "Minecraft.Windows.exe", StringComparison.OrdinalIgnoreCase))
            {
                continue;
            }
            var dst = Path.Combine(destinationDir, rel);
            Directory.CreateDirectory(Path.GetDirectoryName(dst)!);
            CopyFileWithRetry(file, dst);
        }
    }

    static void CopyFileWithRetry(string source, string destination)
    {
        const int maxAttempts = 20;
        for (int attempt = 1; attempt <= maxAttempts; attempt++)
        {
            try
            {
                File.Copy(source, destination, true);
                return;
            }
            catch (UnauthorizedAccessException) when (attempt < maxAttempts)
            {
                Thread.Sleep(250);
            }
            catch (IOException) when (attempt < maxAttempts)
            {
                Thread.Sleep(250);
            }
            catch (UnauthorizedAccessException ex)
            {
                Console.Error.WriteLine($"WARN|Skipping locked/denied file after retries: {source} ({ex.Message})");
                return;
            }
            catch (IOException ex)
            {
                Console.Error.WriteLine($"WARN|Skipping IO-locked file after retries: {source} ({ex.Message})");
                return;
            }
        }
    }

    static async Task CopyExeThroughDesktopPackage(string packageFamilyName, string exeSrc, string exeDst)
    {
        var tmpDir = Path.Combine(Path.GetTempPath(), "modrinth-bedrock-exe");
        Directory.CreateDirectory(tmpDir);
        var tmpExe = Path.Combine(tmpDir, $"Minecraft.Windows.{Guid.NewGuid():N}.exe");
        var tmpPartial = tmpExe + ".tmp";

        var innerScript = $"Copy-Item -LiteralPath '{EscapePs(exeSrc)}' -Destination '{EscapePs(tmpPartial)}' -Force; Move-Item -LiteralPath '{EscapePs(tmpPartial)}' -Destination '{EscapePs(tmpExe)}' -Force";
        var encodedInner = Convert.ToBase64String(Encoding.Unicode.GetBytes(innerScript));
        var desktopArgs = $"-NoProfile -NonInteractive -ExecutionPolicy Bypass -EncodedCommand {encodedInner}";
        var cmd = $"Invoke-CommandInDesktopPackage -PackageFamilyName '{EscapePs(packageFamilyName)}' -App Game -Command 'powershell.exe' -Args '{EscapePs(desktopArgs)}'";

        var psi = new ProcessStartInfo("powershell.exe")
        {
            UseShellExecute = false,
            RedirectStandardError = true,
            RedirectStandardOutput = true,
            CreateNoWindow = true,
            Arguments = $"-NoProfile -NonInteractive -ExecutionPolicy Bypass -Command \"{cmd}\""
        };

        using var proc = Process.Start(psi) ?? throw new Exception("Failed to start PowerShell");
        var stdout = await proc.StandardOutput.ReadToEndAsync();
        var stderr = await proc.StandardError.ReadToEndAsync();
        await proc.WaitForExitAsync();

        if (proc.ExitCode != 0)
        {
            throw new Exception($"Invoke-CommandInDesktopPackage failed: {stderr}\n{stdout}");
        }

        for (int i = 0; i < 300 && !File.Exists(tmpExe); i++)
        {
            await Task.Delay(100);
        }

        if (!File.Exists(tmpExe))
        {
            throw new Exception("Decrypted Minecraft.Windows.exe was not produced");
        }

        Directory.CreateDirectory(Path.GetDirectoryName(exeDst)!);
        if (File.Exists(exeDst)) File.Delete(exeDst);
        File.Move(tmpExe, exeDst);
    }

    static async Task CopyExeWithFallbacks(string packageFamilyName, string exeSrc, string exeDst)
    {
        Exception? desktopErr = null;
        if (!string.IsNullOrWhiteSpace(packageFamilyName))
        {
            try
            {
                await CopyExeThroughDesktopPackage(packageFamilyName, exeSrc, exeDst);
                return;
            }
            catch (Exception ex)
            {
                desktopErr = ex;
                Log("WARN DesktopPackage copy failed, trying direct fallback: " + ex.Message);
                Console.Error.WriteLine("WARN|DesktopPackage copy failed, trying direct fallback");
            }
        }

        if (TryCopyExeDirectWithRetry(exeSrc, exeDst))
        {
            return;
        }

        if (TryCopyExeViaRobocopy(exeSrc, exeDst))
        {
            return;
        }

        if (desktopErr is not null)
        {
            throw new Exception(
                $"Failed to copy Minecraft.Windows.exe via all methods. DesktopPackage error: {desktopErr.Message}"
            );
        }
        throw new Exception("Failed to copy Minecraft.Windows.exe via all methods.");
    }

    static bool TryCopyExeDirectWithRetry(string exeSrc, string exeDst)
    {
        const int maxAttempts = 80;
        for (int attempt = 1; attempt <= maxAttempts; attempt++)
        {
            try
            {
                Directory.CreateDirectory(Path.GetDirectoryName(exeDst)!);
                using var src = new FileStream(
                    exeSrc,
                    FileMode.Open,
                    FileAccess.Read,
                    FileShare.ReadWrite | FileShare.Delete
                );
                using var dst = new FileStream(
                    exeDst,
                    FileMode.Create,
                    FileAccess.Write,
                    FileShare.Read
                );
                src.CopyTo(dst);
                dst.Flush(true);
                if (new FileInfo(exeDst).Length > 0)
                {
                    return true;
                }
            }
            catch (UnauthorizedAccessException) when (attempt < maxAttempts)
            {
                Thread.Sleep(500);
            }
            catch (IOException) when (attempt < maxAttempts)
            {
                Thread.Sleep(500);
            }
            catch
            {
                if (attempt < maxAttempts) Thread.Sleep(500);
            }
        }
        Log("WARN Direct exe copy failed after retries");
        return false;
    }

    static bool TryCopyExeViaRobocopy(string exeSrc, string exeDst)
    {
        try
        {
            var srcDir = Path.GetDirectoryName(exeSrc)!;
            var dstDir = Path.GetDirectoryName(exeDst)!;
            Directory.CreateDirectory(dstDir);
            var fileName = Path.GetFileName(exeSrc);

            var psi = new ProcessStartInfo("robocopy.exe")
            {
                UseShellExecute = false,
                RedirectStandardError = true,
                RedirectStandardOutput = true,
                CreateNoWindow = true,
                Arguments = $"\"{srcDir}\" \"{dstDir}\" \"{fileName}\" /R:8 /W:2 /NFL /NDL /NJH /NJS /NC /NS /NP"
            };

            using var proc = Process.Start(psi);
            if (proc is null)
            {
                return false;
            }
            proc.WaitForExit();
            var code = proc.ExitCode;

            // Robocopy returns <=7 for success-ish states.
            if (code <= 7 && File.Exists(exeDst) && new FileInfo(exeDst).Length > 0)
            {
                return true;
            }

            Log($"WARN Robocopy exe copy failed exit={code}");
            return false;
        }
        catch (Exception ex)
        {
            Log("WARN Robocopy fallback exception: " + ex.Message);
            return false;
        }
    }

    static string EscapePs(string s) => s.Replace("'", "''");
}
