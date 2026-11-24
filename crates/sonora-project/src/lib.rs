pub mod db;
pub mod package;

pub use db::{ProjectDatabase, ProjectRecord};
pub use package::{PackageManager, ProjectManifest};

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_project_db_and_package() {
        let temp_dir = std::env::temp_dir().join("sonora_project_test");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).expect("create temp");

        let db_file = temp_dir.join("catalogue.db");
        let db = ProjectDatabase::open(&db_file.to_string_lossy()).expect("open db");

        let (mut session, project_dir) =
            PackageManager::create_project("My Cool Song", &temp_dir, Some(&db))
                .expect("create project");

        assert!(project_dir.exists());
        assert!(project_dir.join("manifest.json").exists());
        assert!(project_dir.join("session.json").exists());

        // Modify and save
        session.bpm = 130.0;
        PackageManager::save_project(&session, &project_dir, Some(&db)).expect("save");

        // Verify loaded session
        let loaded = PackageManager::load_project(&project_dir).expect("load");
        assert_eq!(loaded.bpm, 130.0);
        assert_eq!(loaded.name, "My Cool Song");

        // Verify DB list
        let list = db.list_all().expect("list");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].bpm, 130.0);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
