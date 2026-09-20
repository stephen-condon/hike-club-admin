//! The real [`AdminStore`], backed by the `HIKES` R2 binding. Pure translation
//! between the trait's byte-blob shape and the Workers R2 API — it can only run
//! inside a deployed/dev worker, so it's excluded from the coverage gate and
//! covered by `wrangler dev --remote` checks instead.
use crate::store::AdminStore;
use worker::Bucket;

pub struct R2Store {
    pub bucket: Bucket,
}

impl AdminStore for R2Store {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        let object = self
            .bucket
            .get(key)
            .execute()
            .await
            .map_err(|e| e.to_string())?;
        match object {
            Some(object) => match object.body() {
                Some(body) => body.bytes().await.map(Some).map_err(|e| e.to_string()),
                None => Ok(None),
            },
            None => Ok(None),
        }
    }

    async fn put(&self, key: &str, body: Vec<u8>, content_type: &str) -> Result<(), String> {
        let metadata = worker::HttpMetadata {
            content_type: Some(content_type.to_string()),
            ..Default::default()
        };
        self.bucket
            .put(key, body)
            .http_metadata(metadata)
            .execute()
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    async fn delete(&self, key: &str) -> Result<(), String> {
        self.bucket.delete(key).await.map_err(|e| e.to_string())
    }

    // @spec STORE-005
    async fn list(&self, prefix: &str) -> Result<Vec<String>, String> {
        let mut keys = Vec::new();
        let mut cursor: Option<String> = None;
        // R2 pages at 1000 keys. The bucket holds two objects per location, so
        // one page covers it today — the loop is here so it stays correct if
        // the club ever outgrows that.
        loop {
            let mut builder = self.bucket.list().prefix(prefix);
            if let Some(cursor) = cursor.take() {
                builder = builder.cursor(cursor);
            }
            let page = builder.execute().await.map_err(|e| e.to_string())?;
            keys.extend(page.objects().into_iter().map(|o| o.key()));
            match page.cursor() {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        Ok(keys)
    }
}
