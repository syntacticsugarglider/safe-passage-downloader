use std::{collections::BTreeMap, convert::TryInto, env, sync::Arc};

use chrono::{DateTime, NaiveDateTime, Utc};
use futures::StreamExt;
use smol::lock::RwLock;
use telegram_bot::{GetFile, PhotoSize};
use tokio::{fs::File, io::AsyncWriteExt};

#[tokio::main]
async fn main() {
    let db = sled::open("../photo_ids").unwrap();
    let photos = db
        .iter()
        .filter_map(|data| {
            data.ok().map(|(time, photo)| {
                (
                    DateTime::from_utc(
                        NaiveDateTime::from_timestamp(
                            i64::from_le_bytes(time.as_ref().try_into().unwrap()),
                            0,
                        ),
                        Utc,
                    ),
                    String::from_utf8(photo.as_ref().to_vec()).unwrap(),
                )
            })
        })
        .collect::<BTreeMap<DateTime<Utc>, String>>();
    let token = env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN not set");
    let api = Arc::new(RwLock::new(telegram_bot::Api::new(token.clone())));
    let token = Arc::new(token);
    futures::stream::iter(photos.into_iter().map(|(date, photo)| {
        let api = api.clone();
        let token = token.clone();
        async move {
            let file = api
                .read()
                .await
                .send(GetFile::new(PhotoSize {
                    width: 1280,
                    file_id: photo,
                    height: 720,
                    file_size: None,
                }))
                .await
                .unwrap();
            let uri = format!(
                "https://api.telegram.org/file/bot{}/{}",
                token.as_str(),
                file.file_path.unwrap()
            );
            let mut file = File::create(format!("photos/{}.jpg", format!("{}", date)))
                .await
                .unwrap();
            let data = surf::get(uri)
                .send()
                .await
                .unwrap()
                .take_body()
                .into_bytes()
                .await
                .unwrap();
            file.write_all(&data).await.unwrap();
        }
    }))
    .buffer_unordered(16)
    .collect::<()>()
    .await;
}
