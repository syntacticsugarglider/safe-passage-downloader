use std::{collections::BTreeMap, convert::TryInto, env, sync::Arc};

use chrono::{
    offset::{TimeZone, Utc},
    DateTime, NaiveDateTime,
};
use futures::StreamExt;
use smol::lock::RwLock;
use telegram_bot::{GetFile, PhotoSize};
use tokio::{fs::File, io::AsyncWriteExt};

#[tokio::main]
async fn main() {
    let db = sled::open("../photo_ids").unwrap();
    //I want to specify which photos to add to the tree --> Which to collect from the iter
    // Local.ymd(y, m, d).hms(h, m, s)
    let last_downloaded = Utc.ymd(2021, 01, 20).and_hms(11, 13, 01); //DateTime::parse_from_rfc3339("2021-01-20 11:13:01 UTC").unwrap(); //fix in a min
    let photos = db
        .iter()
        .filter_map(|data| {
            data.ok().and_then(|(time, photo)| {
                let dt = DateTime::from_utc(
                    NaiveDateTime::from_timestamp(
                        i64::from_le_bytes(time.as_ref().try_into().unwrap()),
                        0,
                    ),
                    Utc,
                );
                //if dt is before last_downloaded, return none
                if dt < last_downloaded {
                    //date before what we want, return none {
                    return None;
                }
                //takes a tuple, returns a tuple to be put into the tree
                Some((dt, String::from_utf8(photo.as_ref().to_vec()).unwrap()))
            })
        })
        .collect::<BTreeMap<DateTime<Utc>, String>>(); //BTree map key has to be an ordered thing, and orders it for you

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
