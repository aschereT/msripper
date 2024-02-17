use std::{fs, path::Path};

use anyhow::Result;
use clap::Parser;
use ffmpeg_sidecar::command::FfmpegCommand;
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

	#[arg(long, long, default_value = "./rips/")]
	path: String,
}

#[tokio::main]
async fn main() {
	let args = Args::parse();
	let path = Path::new(&args.path);
	_ = match fs::create_dir_all(&path) {
		Ok(val) => val,
		Err(e) => panic!("failure creating directory {:#?}", e),
	};
	ffmpeg_sidecar::download::auto_download().unwrap();
	if args.all {
		let albums = match get_all_albums_data().await {
			Ok(val) => val,
			Err(e) => panic!("failure getting albums {:#?}", e),
		};
		println!("{:#?}", albums);
	} else if args.album_id > 0 {
		match get_album(&path, &args.album_id).await {
			Ok(e) => e,
			Err(e) => panic!("{}", e),
		};
	} else if args.song_id > 0 {
		match get_song(&path, &args.song_id).await {
			Ok(e) => e,
			Err(e) => panic!("{}", e),
		};
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

async fn get_all_albums_data() -> Result<AllAlbums, reqwest::Error> {
	let resp = reqwest::get("https://monster-siren.hypergryph.com/api/albums")
		.await?
		.json::<AllAlbums>()
		.await?;
	Ok(resp)
}

#[derive(Deserialize, Debug)]
struct AlbumEntry {
	code: u8,
	msg: String,
	data: AlbumData,
}
#[derive(Deserialize, Debug)]
struct AlbumData {
	cid: String,
	name: String,       //song name
	intro: String,      //description?
	belong: String,     //game?
	coverUrl: String,   //album cover
	coverDeUrl: String, //fancy banner
	songs: Vec<AlbumSongs>,
}
#[derive(Deserialize, Debug)]
struct AlbumSongs {
	cid: String,
	name: String, //song name
	artistes: Vec<String>,
}
async fn get_album(parent_path: &Path, album_id: &u64) -> Result<()> {
	println!("Getting album {}", album_id);
	let album_data = get_album_data(album_id).await?;
	println!("Obtained {:#?} songs", &album_data.data.songs.len());
	let album_path = parent_path.join(album_data.data.name);
	fs::create_dir_all(&album_path)?;
	for song in &album_data.data.songs {
		get_song(&album_path, &song.cid.parse::<u64>().unwrap()).await?;
	}
	// grab cover image
	download_file(
		&album_data.data.coverUrl,
		&album_path.join("cover.jpg"),
	)
	.await?;
	Ok(())
}
async fn get_album_data(album_id: &u64) -> Result<AlbumEntry, reqwest::Error> {
	let url = format!(
		"https://monster-siren.hypergryph.com/api/album/{}/detail",
		album_id
	);
	let resp = reqwest::get(url).await?.json::<AlbumEntry>().await?;
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
async fn get_song(parent_path: &Path, song_id: &u64) -> Result<()> {
	println!("Getting song {}", song_id);
	let song_data: SongEntry = get_song_data(&song_id).await?;
	let out_path = parent_path.join(format!("{}.wav", &song_data.data.name));
	return download_file(&song_data.data.sourceUrl, &out_path).await;
	// TODO: convert to flac
	// TODO: set metadata
}
async fn get_song_data(song_id: &u64) -> Result<SongEntry, reqwest::Error> {
	let url = format!("https://monster-siren.hypergryph.com/api/song/{}", song_id);
	let resp = reqwest::get(url).await?.json::<SongEntry>().await?;
	Ok(resp)
}

async fn download_file(remote_path: &String, local_path: &Path) -> Result<()> {
	let resp = reqwest::get(remote_path).await?.bytes().await?;
	let file_write = fs::write(local_path, resp)?;
	Ok(file_write)
}
fn process_wav(input_path: &String, output_path: &String) {
	FfmpegCommand::new()
		.input(input_path)
		.output(output_path)
		.rawvideo()
		.spawn()
		.unwrap();
}
