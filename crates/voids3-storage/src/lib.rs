//! SQLite stores non-secret state only; OS keychain stores credentials.
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Serialize, de::DeserializeOwned};
use std::{path::Path, sync::Mutex};
use voids3_core::{Credentials, Error, Profile, Result};

pub struct Store {
    db: Mutex<Connection>,
}
impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let db = Connection::open(path).map_err(storage)?;
        db.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(storage)?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS documents (namespace TEXT NOT NULL, id TEXT NOT NULL, value TEXT NOT NULL, PRIMARY KEY(namespace,id));
            PRAGMA user_version=1;").map_err(storage)?;
        Ok(Self { db: Mutex::new(db) })
    }
    pub fn default_location() -> Result<Self> {
        Self::open(&voids3_platform::data_dir()?.join("state.db"))
    }
    pub fn put<T: Serialize>(&self, namespace: &str, id: &str, value: &T) -> Result<()> {
        let json = serde_json::to_string(value).map_err(storage)?;
        self.db.lock().map_err(storage)?.execute("INSERT INTO documents(namespace,id,value) VALUES (?1,?2,?3) ON CONFLICT(namespace,id) DO UPDATE SET value=excluded.value", params![namespace,id,json]).map_err(storage)?;
        Ok(())
    }
    pub fn get<T: DeserializeOwned>(&self, namespace: &str, id: &str) -> Result<Option<T>> {
        let json: Option<String> = self
            .db
            .lock()
            .map_err(storage)?
            .query_row(
                "SELECT value FROM documents WHERE namespace=?1 AND id=?2",
                params![namespace, id],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?;
        json.map(|j| serde_json::from_str(&j).map_err(storage))
            .transpose()
    }
    pub fn list<T: DeserializeOwned>(&self, namespace: &str) -> Result<Vec<T>> {
        let db = self.db.lock().map_err(storage)?;
        let mut statement = db
            .prepare("SELECT value FROM documents WHERE namespace=?1 ORDER BY id")
            .map_err(storage)?;
        let rows = statement
            .query_map([namespace], |r| r.get::<_, String>(0))
            .map_err(storage)?;
        rows.map(|r| serde_json::from_str(&r.map_err(storage)?).map_err(storage))
            .collect()
    }
    pub fn delete(&self, namespace: &str, id: &str) -> Result<()> {
        self.db
            .lock()
            .map_err(storage)?
            .execute(
                "DELETE FROM documents WHERE namespace=?1 AND id=?2",
                params![namespace, id],
            )
            .map_err(storage)?;
        Ok(())
    }
    pub fn profiles(&self) -> Result<Vec<Profile>> {
        self.list("profiles")
    }
    pub fn profile(&self, id: &str) -> Result<Profile> {
        self.get("profiles", id)?
            .ok_or_else(|| Error::Invalid("Connection profile not found".into()))
    }
    pub fn save_profile(&self, profile: &Profile, credentials: Option<&Credentials>) -> Result<()> {
        profile.validate()?;
        if let Some(credentials) = credentials {
            credentials.validate()?;
            // Keychain write first: a failure never creates a deceptively usable profile.
            let secret = serde_json::json!({"access_key": credentials.access_key, "secret_key": credentials.secret_key, "session_token": credentials.session_token}).to_string();
            credential_entry(&profile.id)?
                .set_password(&secret)
                .map_err(credential_error)?;
        } else if self.get::<Profile>("profiles", &profile.id)?.is_none() {
            return Err(Error::Invalid(
                "Credentials are required for a new connection".into(),
            ));
        }
        self.put("profiles", &profile.id, profile)
    }
    pub fn credentials(&self, id: &str) -> Result<Credentials> {
        let secret = credential_entry(id)?
            .get_password()
            .map_err(credential_error)?;
        serde_json::from_str(&secret).map_err(|_| {
            Error::Credentials(
                "Stored credential format is invalid; save the connection again".into(),
            )
        })
    }
    pub fn delete_profile(&self, id: &str) -> Result<()> {
        match credential_entry(id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(e) => return Err(credential_error(e)),
        }
        self.delete("profiles", id)
    }
}
fn credential_entry(id: &str) -> Result<keyring::Entry> {
    if uuid_invalid(id) {
        return Err(Error::Invalid("Invalid credential reference".into()));
    }
    keyring::Entry::new("dev.voids3.connections", id).map_err(credential_error)
}
fn uuid_invalid(id: &str) -> bool {
    id.len() != 36 || !id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}
fn credential_error(e: impl std::fmt::Display) -> Error {
    Error::Credentials(e.to_string())
}
fn storage(e: impl std::fmt::Display) -> Error {
    Error::Storage(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn state_survives_restart_and_namespaces_are_isolated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        {
            let store = Store::open(&path).unwrap();
            store.put("queue", "one", &vec![1, 2, 3]).unwrap();
        }
        let store = Store::open(&path).unwrap();
        assert_eq!(
            store.get::<Vec<i32>>("queue", "one").unwrap(),
            Some(vec![1, 2, 3])
        );
        assert_eq!(store.get::<Vec<i32>>("profiles", "one").unwrap(), None);
        store.delete("queue", "one").unwrap();
        assert!(store.list::<Vec<i32>>("queue").unwrap().is_empty());
    }
}
