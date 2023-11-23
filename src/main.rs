use std::fs;

use anyhow::Result;
use clap::Parser;
use serde::Deserialize;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = 0)]
    album_id: u64,

    #[arg(short, long, default_value_t = 0)]
    song_id: u64,

    #[arg(long, long, action)]
    all: bool,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    if args.all {
        let albums = match get_all_albums().await {
            Ok(val) => val,
            Err(e) => panic!("failure getting albums {:#?}", e),
        };
        println!("{:#?}", albums);
    } else if args.album_id > 0 {
        println!("run album {}", args.album_id);
    } else if args.song_id > 0 {
        println!("run song {}", args.song_id);
        let song_data = match get_song(args.song_id).await {
            Ok(wav) => wav,
            Err(e) => panic!("{}", e),
        };
        // TODO: write to file
        let dl_resp = download_file(song_data.data.sourceUrl, "/tmp/test.wav".to_string()).await;
        println!("{:#?}", dl_resp);
    } else {
        // TODO: error out
        panic!("missing arguments")
    }
}

#[derive(Deserialize, Debug)]
struct AllAlbums {
    code: u8,
    msg: String,
    data: Vec<AllAlbumsEntry>,
}
#[derive(Deserialize, Debug)]
struct AllAlbumsEntry {
    cid: String,
    name: String,
    coverUrl: String,
    artistes: Vec<String>,
}

async fn get_all_albums() -> Result<AllAlbums, reqwest::Error> {
    let resp = reqwest::get("https://monster-siren.hypergryph.com/api/albums")
        .await?
        .json::<AllAlbums>()
        .await?;
    Ok(resp)
}

#[derive(Deserialize, Debug)]
struct SongEntry {
    code: u8,
    msg: String,
    data: SongData,
}
#[derive(Deserialize, Debug)]
struct SongData {
    cid: String,
    name: String, //song name
    albumCid: String,
    sourceUrl: String, //download url
    lyricUrl: Option<String>,
    mvUrl: Option<String>,
    mvCoverUrl: Option<String>,
    artists: Vec<String>,
}
async fn get_song(song_id: u64) -> Result<SongEntry, reqwest::Error> {
    let url = format!("https://monster-siren.hypergryph.com/api/song/{}", song_id);
    let resp = reqwest::get(url).await?.json::<SongEntry>().await?;
    Ok(resp)
}

async fn download_file(remote_path: String, local_path: String) -> Result<()> {
    let resp = reqwest::get(remote_path).await?.bytes().await?;
    let file_write = fs::write(local_path, resp)?;
    Ok(file_write)
}
