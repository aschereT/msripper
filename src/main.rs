use std::{
	fs,
	path::{Path, PathBuf},
};

use anyhow::{Error, Result};
use clap::Parser;
use ffmpeg_sidecar::{
	command::FfmpegCommand,
	event::FfmpegEvent,
};
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
	match fs::create_dir_all(&path) {
		Ok(val) => val,
		Err(e) => panic!("failure creating directory {:#?}", e),
	};
	let canon_path = &path.canonicalize().unwrap();
	ffmpeg_sidecar::download::auto_download().unwrap();

	if args.all {
		match get_all_albums(&canon_path).await {
			Ok(e) => e,
			Err(e) => panic!("failure getting all albums {}", e),
		};
	} else if args.album_id > 0 {
		match get_album(&canon_path, &args.album_id).await {
			Ok(e) => e,
			Err(e) => panic!("failure getting album {}", e),
		};
	} else if args.song_id > 0 {
		match get_song(&canon_path, &args.song_id, None).await {
			Ok(e) => e,
			Err(e) => panic!("failure getting song {}", e),
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
async fn get_all_albums(parent_path: &Path) -> Result<()> {
	let albums = match get_all_albums_data().await {
		Ok(val) => val,
		Err(e) => panic!("failure getting albums {:#?}", e),
	};
	for album in albums.data {
		get_album(parent_path, &album.cid.parse::<u64>().unwrap()).await?;
	}
	Ok(())
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
#[derive(Deserialize, Clone, Debug)]
struct AlbumData {
	cid: String,
	name: String,       //song name
	intro: String,      //description?
	belong: String,     //game?
	coverUrl: String,   //album cover
	coverDeUrl: String, //fancy banner
	songs: Vec<AlbumSongs>,
}
#[derive(Deserialize, Clone, Debug)]
struct AlbumSongs {
	cid: String,
	name: String, //song name
	artistes: Vec<String>,
}
async fn get_album(parent_path: &Path, album_id: &u64) -> Result<()> {
	println!("Getting album {}", album_id);
	let album_data = get_album_data(album_id).await?;
	println!("Obtained {:#?} songs", &album_data.data.songs.len());
	let album_path = parent_path.join(&album_data.data.name);
	fs::create_dir_all(&album_path)?;
	// grab cover image
	let cover_path = album_path.join("cover.jpg");
	download_file(&album_data.data.coverUrl, &cover_path).await?;
	for song in &album_data.data.songs {
		get_song(
			&album_path,
			&song.cid.parse::<u64>().unwrap(),
			Some(&album_data.data),
		)
		.await?;
	}
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
async fn get_song(
	parent_path: &Path,
	song_id: &u64,
	album: Option<&AlbumData>,
) -> Result<Box<PathBuf>, Error> {
	println!("Getting song {}", song_id);
	let song_data: SongEntry = get_song_data(&song_id).await?;
	let out_path = parent_path.join(&song_data.data.name).with_extension("wav");
	if Path::exists(&out_path.with_extension("flac")) {
		println!("Already exists, skipping");
		return Ok(Box::new(out_path.with_extension("flac")));
	};
	download_file(&song_data.data.sourceUrl, &out_path).await?;
	let cover_path = parent_path.join("cover.jpg");
	let album_data = match album {
		Some(a) => a.clone(),
		None => {
			let x = get_album_data(&song_data.data.albumCid.parse::<u64>().unwrap())
				.await
				.unwrap();
			download_file(&x.data.coverUrl, &cover_path).await?;
			x.data
		}
	};
	process_wav(
		&out_path,
		&out_path.with_extension("flac"),
		&cover_path,
		&album_data,
		&song_data.data,
	);
	fs::remove_file(&out_path)?;
	return Ok(Box::new(out_path.with_extension("flac")));
}
async fn get_song_data(song_id: &u64) -> Result<SongEntry, reqwest::Error> {
	let url = format!("https://monster-siren.hypergryph.com/api/song/{}", song_id);
	let resp = reqwest::get(url).await?.json::<SongEntry>().await?;
	Ok(resp)
}

async fn download_file(remote_path: &String, local_path: &Path) -> Result<()> {
	println!("Downloading {} to {}", remote_path, local_path.display());
	if Path::exists(local_path) {
		println!("Already exists, skipping");
		return Ok(());
	};
	let resp = reqwest::get(remote_path).await?.bytes().await?;
	let file_write = fs::write(local_path, resp)?;
	Ok(file_write)
}

fn process_wav(
	input_path: &Path,
	output_path: &Path,
	cover_path: &Path,
	album: &AlbumData,
	song: &SongData,
) {
	println!(
		"process_wav input_path={}, output_path={}, cover_path={}",
		input_path.display(),
		output_path.display(),
		cover_path.display(),
	);
	let mut runner = FfmpegCommand::new()
		.input(input_path.to_str().unwrap())
		.input(cover_path.to_str().unwrap())
		.hide_banner()
		.no_overwrite()
		.args(["-metadata", format!("title={}", &song.name).as_str()])
		.args(["-metadata", format!("album={}", &album.name).as_str()])
		// TODO: add lyrics
		.args([
			"-metadata",
			format!(
				"lyrics={}",
				song.lyricUrl.as_ref().unwrap_or(&String::default())
			)
			.as_str(),
		])
		.args([
			"-metadata",
			format!("artist={}", song.artists.join(",")).as_str(),
		])
		.args([
			"-metadata",
			format!("comment=albumCid {}, cid {}", &album.cid, &song.cid).as_str(),
		])
		.args(["-disposition:v", "attached_pic"])
		.output(output_path.to_str().unwrap())
		.print_command()
		.spawn()
		.unwrap();
	runner.iter().unwrap().for_each(|e| match e {
		// FfmpegEvent::Progress(FfmpegProgress { frame, .. }) => println!("Current frame: {frame}"),
		FfmpegEvent::Log(_level, msg) => println!("[ffmpeg] {msg}"),
		_ => {}
	});
}
