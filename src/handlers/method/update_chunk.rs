use reqwest::Client;
use serde::Deserialize;
use zenth_dto::{UpdateChunkRequest, UpdateChunkResponse};

const MAX_CHUNK_SIZE: u32 = 512 * 1024; // 512 Ko max par chunk

#[derive(Deserialize)]
struct PlatformEntry {
    file: String,
    #[allow(dead_code)] sha256: String,
    size: u64,
    #[allow(dead_code)] signature: String,
}

#[derive(Deserialize)]
struct Manifest {
    #[allow(dead_code)] version: String,
    #[allow(dead_code)] notes: String,
    #[serde(flatten)]
    platforms: std::collections::HashMap<String, PlatformEntry>,
}

pub async fn get_update_chunk(
    req: UpdateChunkRequest,
    rustfs_base_url: &str,
) -> Result<UpdateChunkResponse, String> {
    // current_version contient la plateforme (workaround proto legacy)
    let platform = if !req.current_version.is_empty() {
        req.current_version.clone()
    } else {
        "linux-x86_64-deb".to_string()
    };

    let client = Client::new();

    // Récupère le manifest pour connaître le nom du fichier et la taille totale
    let manifest_url = format!("{}/manifest.json", rustfs_base_url);
    let manifest: Manifest = client
        .get(&manifest_url)
        .send()
        .await
        .map_err(|e| format!("Manifest inaccessible : {}", e))?
        .json()
        .await
        .map_err(|e| format!("Manifest JSON invalide : {}", e))?;

    let entry = manifest.platforms.get(&platform)
        .ok_or_else(|| format!("Plateforme '{}' absente du manifest", platform))?;

    let total = entry.size;
    let chunk_size = req.chunk_size.min(MAX_CHUNK_SIZE) as u64;
    let end = (req.offset + chunk_size - 1).min(total - 1);

    // Structure plate dans RustFS : {base}/{file} (pas de sous-dossier par plateforme)
    let file_url = format!("{}/{}", rustfs_base_url, entry.file);

    let data = client
        .get(&file_url)
        .header("Range", format!("bytes={}-{}", req.offset, end))
        .send()
        .await
        .map_err(|e| format!("Téléchargement chunk échoué : {}", e))?
        .bytes()
        .await
        .map_err(|e| format!("Lecture bytes échouée : {}", e))?
        .to_vec();

    let is_last = req.offset + data.len() as u64 >= total;

    Ok(UpdateChunkResponse {
        offset: req.offset,
        data,
        is_last,
        total,
    })
}
