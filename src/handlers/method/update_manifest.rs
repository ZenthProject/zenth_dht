use reqwest::Client;
use serde::Deserialize;
use zenth_dto::{UpdateManifestRequest, UpdateManifestResponse};

#[derive(Deserialize)]
struct PlatformEntry {
    #[allow(dead_code)]
    file:      String,
    sha256:    String,
    size:      u64,
    signature: String,
}

#[derive(Deserialize)]
struct Manifest {
    version: String,
    notes:   String,
    #[serde(flatten)]
    platforms: std::collections::HashMap<String, PlatformEntry>,
}

pub async fn get_update_manifest(
    req: UpdateManifestRequest,
    rustfs_base_url: &str,
) -> Result<UpdateManifestResponse, String> {
    // req.platform est le champ canonique ; req.current_version = fallback legacy
    let platform = if !req.platform.is_empty() {
        req.platform.clone()
    } else if !req.current_version.is_empty() {
        req.current_version.clone()
    } else {
        "linux-x86_64-deb".to_string()
    };

    let manifest_url = format!("{}/manifest.json", rustfs_base_url);

    let client = Client::new();
    let resp = client
        .get(&manifest_url)
        .send()
        .await
        .map_err(|e| format!("Manifest inaccessible (RustFS) : {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("RustFS a retourné HTTP {} pour manifest.json", resp.status()));
    }

    let manifest: Manifest = resp
        .json()
        .await
        .map_err(|e| format!("Manifest JSON invalide : {}", e))?;

    let entry = manifest.platforms.get(&platform).ok_or_else(|| {
        format!("Plateforme '{}' absente du manifest", platform)
    })?;

    use base64::Engine;
    let signature = base64::engine::general_purpose::STANDARD
        .decode(&entry.signature)
        .map_err(|e| format!("Signature base64 invalide : {}", e))?;

    Ok(UpdateManifestResponse {
        latest_version: manifest.version,
        sha256:         entry.sha256.clone(),
        size:           entry.size,
        signature,
        notes:          manifest.notes,
        platform,
    })
}
