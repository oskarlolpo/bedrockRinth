//! Helpers for patching and inspecting Appx manifests for Bedrock multi-instance support.
//!
//! This module updates manifest capabilities and trusted-launch settings used by the launcher.
//! It keeps behavior compatible with existing UWP requirements and SCCD generation.
//!
//! Reference: https://github.com/MicrosoftDocs/windows-dev-docs/blob/docs/uwp/launch-resume/multi-instance-uwp.md

use futures_util::TryFutureExt;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;
use tracing::debug;
use xmltree::{AttributeMap, Element, EmitterConfig, Namespace, XMLNode};

const SCCD_XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<CustomCapabilityDescriptor xmlns="http://schemas.microsoft.com/appx/2018/sccd" xmlns:s="http://schemas.microsoft.com/appx/2018/sccd">
  <CustomCapabilities>
    <CustomCapability Name="Microsoft.coreAppActivation_8wekyb3d8bbwe"></CustomCapability>
  </CustomCapabilities>
  <AuthorizedEntities AllowAny="true"/>
  <Catalog>FFFF</Catalog>
</CustomCapabilityDescriptor>
"#;

/// е†™е…Ґ SCCD ж–‡д»¶
pub fn write_sccd(dir: &Path) -> io::Result<()> {
    let path = dir.join("CustomCapability.SCCD");
    fs::write(&path, strip_bom(SCCD_XML).as_bytes())?;
    Ok(())
}

/// еЋ»й™¤ UTF-8 BOM
fn strip_bom(s: &str) -> &str {
    const BOM: &str = "\u{feff}";
    s.strip_prefix(BOM).unwrap_or(s)
}

fn has_xmlns_prefix(attrs: &AttributeMap<String, String>, prefix: &str) -> bool {
    let key = format!("xmlns:{}", prefix);
    attrs.contains_key(&key)
}

/// иЎҐдёЃжё…еЌ•ж–‡д»¶д»Ґж”ЇжЊЃ UWP е¤љејЂе’Њи„±з¦»жІ™з›’иїђиЎЊ
pub fn patch_manifest(dir: &Path) -> io::Result<bool> {
    let manifest_path = dir.join("AppxManifest.xml");
    if !manifest_path.exists() {
        return Ok(false);
    }

    // 1. иЇ»еЏ–е№¶еЋ»й™¤ BOM
    let mut xml_str = String::new();
    File::open(&manifest_path)?.read_to_string(&mut xml_str)?;
    let xml_str = strip_bom(&xml_str);

    // 2. и§Јжћђдёє XML ж ‘пјЊж №е…ѓзґ еЌі <Package>
    let mut pkg =
        Element::parse(xml_str.as_bytes()).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    // 3. ењЁ pkg.namespaces дё­иЎҐе…Ё xmlns е‰ЌзјЂпјЊйЃїе…Ќй‡Ќе¤Ќ
    let ns = pkg.namespaces.get_or_insert_with(Namespace::empty);

    // ж·»еЉ з”ЁдєЋе¤љејЂзљ„ desktop4 е‘ЅеђЌз©єй—ґ
    if !ns.0.contains_key("desktop4") {
        ns.0.insert(
            "desktop4".to_string(),
            "http://schemas.microsoft.com/appx/manifest/desktop/windows10/4".to_string(),
        );
    }
    // дїќз•™еЋџжњ‰зљ„е‘ЅеђЌз©єй—ґ
    if !ns.0.contains_key("uap4") {
        ns.0.insert(
            "uap4".to_string(),
            "http://schemas.microsoft.com/appx/manifest/uap/windows10/4".to_string(),
        );
    }
    if !ns.0.contains_key("rescap") {
        ns.0.insert(
            "rescap".to_string(),
            "http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities".to_string(),
        );
    }
    if !ns.0.contains_key("uap10") {
        ns.0.insert(
            "uap10".to_string(),
            "http://schemas.microsoft.com/appx/manifest/uap/windows10/10".to_string(),
        );
    }

    // еђ€е№¶ IgnorableNamespaces е±ћжЂ§пјЊеЉ е…Ґ desktop4
    let required = ["uap", "uap4", "uap10", "rescap", "desktop4"];
    pkg.attributes
        .entry("IgnorableNamespaces".into())
        .and_modify(|v| {
            let mut parts: HashSet<_> = v.split_whitespace().collect();
            for &p in &required {
                parts.insert(p);
            }
            *v = parts.into_iter().collect::<Vec<_>>().join(" ");
        })
        .or_insert_with(|| required.join(" "));

    // 4. ж›ґж–° <Applications>пјЊж·»еЉ е¤љејЂж”ЇжЊЃ
    if let Some(apps) = pkg.get_mut_child("Applications") {
        // з§»й™¤е¤љдЅ™е±ћжЂ§
        apps.attributes.remove("uap10:TrustLevel");
        // зЎ®дїќжЇЏдёЄ Application иЉ‚з‚№йѓЅжњ‰ TrustLevel е’Њ SupportsMultipleInstances
        for child in apps.children.iter_mut().filter_map(|n| match n {
            XMLNode::Element(e) => Some(e),
            _ => None,
        }) {
            if child.name == "Application" {
                // ж·»еЉ и„±з¦»жІ™з›’зљ„ TrustLevel
                child
                    .attributes
                    .entry("uap10:TrustLevel".into())
                    .or_insert_with(|| "mediumIL".into());
                // ж·»еЉ е¤љејЂж”ЇжЊЃ
                child.attributes.insert(
                    "desktop4:SupportsMultipleInstances".to_string(),
                    "true".to_string(),
                );
            }
        }
    }

    // 5. й‡Ќе»є <Capabilities>пјЊйЎєеєЏпјљ[Capability*] в†’ [rescap:Capability*] в†’ [uap4:CustomCapability*] в†’ [DeviceCapability*]
    if let Some(caps) = pkg.get_mut_child("Capabilities") {
        // 1) жЉЉеЋџ children дёЂж¬ЎжЂ§ж‹їе‡є
        let old = std::mem::take(&mut caps.children);

        // 2) е€†з±»е€°е››з»„
        let mut group1 = Vec::new(); // <Capability>
        let mut group3 = Vec::new(); // <rescap:Capability>
        let mut group4 = Vec::new(); // <uap4:CustomCapability>
        let mut group2 = Vec::new(); // <DeviceCapability>
        for node in old {
            match &node {
                XMLNode::Element(e) if e.name == "Capability" => {
                    group1.push(node.clone());
                }
                XMLNode::Element(e) if e.name == "rescap:Capability" => {
                    group3.push(node.clone());
                }
                XMLNode::Element(e) if e.name == "uap4:CustomCapability" => {
                    group4.push(node.clone());
                }
                XMLNode::Element(e) if e.name == "DeviceCapability" => {
                    group2.push(node.clone());
                }
                _ => {
                    group1.push(node.clone());
                }
            }
        }

        // 3) зЎ®дїќ runFullTrust е’Њ uap4 и‡Єе®љд№‰е­ењЁ
        let ensure = |grp: &mut Vec<XMLNode>, tag: &str, name: &str| {
            if !grp.iter().any(|n| match n {
                XMLNode::Element(e) => {
                    e.name == tag && e.attributes.get("Name") == Some(&name.to_string())
                }
                _ => false,
            }) {
                let mut e = Element::new(tag);
                e.attributes.insert("Name".into(), name.into());
                grp.push(XMLNode::Element(e));
            }
        };
        ensure(&mut group3, "rescap:Capability", "runFullTrust");
        ensure(
            &mut group4,
            "uap4:CustomCapability",
            "Microsoft.coreAppActivation_8wekyb3d8bbwe",
        );

        // 4) жё…з©єе№¶жЊ‰ж–°йЎєеєЏж‹је›ћ
        caps.children.clear();
        caps.children.extend(group1);
        caps.children.extend(group3);
        caps.children.extend(group4);
        caps.children.extend(group2);
    } else {
        // и‹ҐдёЂејЂе§‹жІЎжњ‰ <Capabilities>пјЊе€™жЊ‰еђЊдёЂйЎєеєЏе€›е»є
        let mut caps = Element::new("Capabilities");
        // group3: runFullTrust
        caps.children.push(XMLNode::Element({
            let mut e = Element::new("rescap:Capability");
            e.attributes.insert("Name".into(), "runFullTrust".into());
            e
        }));
        // group4: uap4 и‡Єе®љд№‰
        caps.children.push(XMLNode::Element({
            let mut e = Element::new("uap4:CustomCapability");
            e.attributes.insert(
                "Name".into(),
                "Microsoft.coreAppActivation_8wekyb3d8bbwe".into(),
            );
            e
        }));
        pkg.children.push(XMLNode::Element(caps));
    }

    // 6. жё…зђ†и‡Єй—­еђ€иЉ‚з‚№
    for node in pkg.children.iter_mut() {
        if let XMLNode::Element(elem) = node {
            if matches!(
                elem.name.as_str(),
                "Identity" | "PhoneIdentity" | "TargetDeviceFamily" | "PackageDependency"
            ) {
                elem.children.clear();
            }
        }
    }

    // 7. еєЏе€—еЊ–иѕ“е‡єе№¶ж јејЏеЊ–пј€з»џдёЂ CRLF жЌўиЎЊе’Њи‡Єй—­еђ€пј‰
    let mut out = Vec::new();
    let cfg = EmitterConfig::new()
        .perform_indent(true)
        .write_document_declaration(true)
        .normalize_empty_elements(true)
        .line_separator("\r\n");
    pkg.write_with_config(&mut out, cfg)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    fs::write(&manifest_path, out)?;

    // 8. е†™е…Ґ SCCD
    write_sccd(dir)?;
    Ok(true)
}

/// иЋ·еЏ–еЊ…дїЎжЃЇ
pub fn get_package_info(
    app_user_model_id: &str,
) -> windows::core::Result<Option<(String, String, String)>> {
    match windows::ApplicationModel::AppInfo::GetFromAppUserModelId(&app_user_model_id.into()) {
        Ok(app_info) => match app_info.Package() {
            Ok(package) => {
                let version = if let Ok(version) = package.Id().and_then(|id| id.Version()) {
                    Some(format!(
                        "{}.{}.{}.{}",
                        version.Major, version.Minor, version.Build, version.Revision
                    ))
                } else {
                    None
                };

                let package_family_name =
                    if let Ok(package_family_name) = package.Id().and_then(|id| id.FamilyName()) {
                        Some(package_family_name)
                    } else {
                        return Err(windows::core::Error::from(io::Error::new(
                            io::ErrorKind::Other,
                            "ж— жі•иЋ·еЏ–еЊ…е®¶ж—ЏеђЌз§°",
                        )));
                    };

                let package_full_name =
                    if let Ok(package_full_name) = package.Id().and_then(|id| id.FullName()) {
                        Some(package_full_name.to_string())
                    } else {
                        return Err(windows::core::Error::from(io::Error::new(
                            io::ErrorKind::Other,
                            "ж— жі•иЋ·еЏ–еЊ…е…ЁеђЌ",
                        )));
                    };

                Ok(Some((
                    version.unwrap(),
                    package_family_name.unwrap().to_string(),
                    package_full_name.unwrap(),
                )))
            }
            Err(err) => Err(err.into()),
        },
        Err(err) => Err(err.into()),
    }
}

/// еј‚ж­ҐиЋ·еЏ–жё…еЌ•дё­зљ„ Identity дїЎжЃЇ
pub async fn get_manifest_identity(appx_path: &str) -> Result<(String, String), String> {
    let manifest_path = Path::new(appx_path).join("AppxManifest.xml");
    debug!("Manifest и·Їеѕ„: {}", manifest_path.display());

    // еј‚ж­ҐиЇ»еЏ–пј€зЎ®дїќдЅїз”Ё tokio::fsпј‰
    let xml = tokio::fs::read_to_string(&manifest_path)
        .map_err(|e| format!("ж— жі•ж‰“ејЂ/иЇ»еЏ–ж–‡д»¶ {}: {}", manifest_path.display(), e))
        .await?;

    // ж‰ѕе€°з¬¬дёЂдёЄ <Identity ...> ж€– <Identity/...>
    let start_idx = match xml.find("<Identity") {
        Some(i) => i,
        None => return Err("жњЄж‰ѕе€° <Identity> иЉ‚з‚№".to_string()),
    };
    // ж‰ѕе€°ж ‡з­ѕз»“жќџз¬¦еЏ· '>'пј€еЊ…ж‹¬и‡Єй—­еђ€ "/>" жѓ…е†µпј‰
    let rest = &xml[start_idx..];
    let end_rel = rest.find('>').ok_or("ж— жі•е®љдЅЌ Identity ж ‡з­ѕз»“жќџ")?;
    let tag = &rest[..=end_rel]; // еЊ…еђ« '>'

    // з®ЂеЌ•е±ћжЂ§жЏђеЏ–е™Ёпј€ж”ЇжЊЃеЏЊеј•еЏ·ж€–еЌ•еј•еЏ·пј‰
    fn extract_attr(tag: &str, key: &str) -> Option<String> {
        let needle = format!("{}=", key);
        let pos = tag.find(&needle)?;
        let after = &tag[pos + needle.len()..].trim_start();
        let mut chars = after.chars();
        let first = chars.next()?;
        if first == '"' || first == '\'' {
            let quote = first;
            let mut val = String::new();
            for c in chars {
                if c == quote {
                    return Some(val);
                }
                val.push(c);
            }
            None
        } else {
            // йќћеј•еЏ·жѓ…е†µпј€зЅ•и§Ѓпј‰вЂ”вЂ”иЇ»е€°з©єз™Ѕж€–'>'ж€–'/'
            let val: String = after
                .chars()
                .take_while(|c| !c.is_whitespace() && *c != '>' && *c != '/')
                .collect();
            if val.is_empty() {
                None
            } else {
                Some(val)
            }
        }
    }

    let name = extract_attr(tag, "Name");
    let version = extract_attr(tag, "Version");

    match (name, version) {
        (Some(n), Some(v)) => {
            debug!("и§Јжћђз»“жћњ => Name: {}, Version: {}", n, v);
            Ok((n, v))
        }
        _ => Err("жњЄж‰ѕе€° Identity зљ„ Name ж€– Version".to_string()),
    }
}

