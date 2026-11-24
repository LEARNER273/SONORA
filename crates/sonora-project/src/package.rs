use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::db::{ProjectDatabase, ProjectRecord};
use sonora_core::SongSession;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectManifest {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub bpm: f32,
    pub created_at: i64,
    pub updated_at: i64,
}

pub struct PackageManager;

impl PackageManager {
    pub fn create_project(
        name: &str,
        parent_dir: &Path,
        db: Option<&ProjectDatabase>,
    ) -> Result<(SongSession, PathBuf), String> {
        let session = SongSession::new(name);
        let safe_name = name.replace(|c: char| !c.is_alphanumeric() && c != '_' && c != '-', "_");
        let project_dir = parent_dir.join(format!("{}.sonora", safe_name));

        // Create directories
        fs::create_dir_all(&project_dir).map_err(|e| e.to_string())?;
        fs::create_dir_all(project_dir.join("audio")).map_err(|e| e.to_string())?;
        fs::create_dir_all(project_dir.join("backups")).map_err(|e| e.to_string())?;

        // Save initial files
        Self::save_project(&session, &project_dir, db)?;

        Ok((session, project_dir))
    }

    pub fn save_project(
        session: &SongSession,
        project_dir: &Path,
        db: Option<&ProjectDatabase>,
    ) -> Result<(), String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let manifest = ProjectManifest {
            schema_version: 1,
            id: session.id.clone(),
            name: session.name.clone(),
            bpm: session.bpm,
            created_at: now,
            updated_at: now,
        };

        // Write manifest.json
        let manifest_json = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
        fs::write(project_dir.join("manifest.json"), manifest_json).map_err(|e| e.to_string())?;

        // Write session.json (atomic write via temp file)
        let session_json = serde_json::to_string_pretty(&session).map_err(|e| e.to_string())?;
        let temp_session = project_dir.join("session.json.tmp");
        fs::write(&temp_session, session_json).map_err(|e| e.to_string())?;
        fs::rename(temp_session, project_dir.join("session.json")).map_err(|e| e.to_string())?;

        // Update database if provided
        if let Some(database) = db {
            let record = ProjectRecord {
                id: session.id.clone(),
                name: session.name.clone(),
                path: project_dir.to_string_lossy().to_string(),
                bpm: session.bpm,
                track_count: session.tracks.len(),
                created_at: now,
                updated_at: now,
            };
            let _ = database.insert_or_update(&record);
        }

        Ok(())
    }

    pub fn load_project(project_dir: &Path) -> Result<SongSession, String> {
        let session_file = project_dir.join("session.json");
        if !session_file.exists() {
            return Err(format!("session.json not found in {:?}", project_dir));
        }

        let content = fs::read_to_string(&session_file).map_err(|e| e.to_string())?;
        let session: SongSession = serde_json::from_str(&content).map_err(|e| e.to_string())?;
        Ok(session)
    }

    pub fn autosave(session: &SongSession, project_dir: &Path) -> Result<PathBuf, String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let backup_dir = project_dir.join("backups");
        fs::create_dir_all(&backup_dir).map_err(|e| e.to_string())?;

        let backup_file = backup_dir.join(format!("backup_{}.json", now));
        let session_json = serde_json::to_string_pretty(&session).map_err(|e| e.to_string())?;
        fs::write(&backup_file, session_json).map_err(|e| e.to_string())?;

        Ok(backup_file)
    }
}
