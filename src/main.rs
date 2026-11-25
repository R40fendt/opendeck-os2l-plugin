use openaction::*;
use std::sync::Mutex;

use std::thread::sleep;
use std::{io::Write, net::TcpStream, thread, time::Duration};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct ButtonSettings {
	channel: u32,
	value: i16,
}
impl Default for ButtonSettings {
	fn default() -> Self {
		Self { channel: 1, value: 0 }
	}
}

#[derive(Serialize)]
#[serde(tag = "evt", rename_all = "lowercase")]
enum Os2l {
    Beat { change: bool, pos: u32, bpm: f64, strength: f64 },
    Btn { name: String, state: String },   // "on" / "off"
    Cmd { id: u32, param: f64 },
}

static CONN: Mutex<Option<TcpStream>> = Mutex::new(None);

fn send(msg: &Os2l) -> std::io::Result<()> {
	let data = serde_json::to_string(msg).unwrap();
	let mut guard = CONN.lock().unwrap();

	// erst die bestehende Verbindung probieren
	if let Some(stream) = guard.as_mut() {
		if stream.write_all(data.as_bytes()).and_then(|_| stream.flush()).is_ok() {
			return Ok(());
		}
	}
	// sonst einmal neu verbinden
	let mut stream = TcpStream::connect("127.0.0.1:9996")?;
	stream.set_nodelay(true)?;
	stream.write_all(data.as_bytes())?;
	stream.flush()?;
	*guard = Some(stream);
	Ok(())
}




async fn up(
	instance: &Instance,
	settings: &ButtonSettings,
	channel: u32,
) -> OpenActionResult<()> {
	let mut clone = settings.clone();
	clone.value = if settings.value.clone()>0 { 0 } else { 255 };
	instance.set_state(if clone.value>0 { 1 } else { 0 }).await?;
	instance.set_settings(&clone).await?;
 	let msg = Os2l::Cmd {
		id: channel,
		param: 0.into(),
	};
	if let Err(e) = send(&msg) {
		eprintln!("OS2L senden fehlgeschlagen: {e}");
	}

	Ok(())
}
async fn down(
	channel: u32,
) -> OpenActionResult<()> {
 	let msg = Os2l::Cmd {
		id: channel,
		param:  255.into(),
	};
	if let Err(e) = send(&msg) {
		eprintln!("OS2L senden fehlgeschlagen: {e}");
	}

	Ok(())
}

struct OS2LAction;

#[async_trait]
impl Action for OS2LAction{
	const UUID: ActionUuid = "de.ritzenbergen.os2l.action";
	type Settings = ButtonSettings;

	async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		up(instance, settings, settings.channel).await
	}

	async fn key_down(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
		down(settings.channel).await
	}

	async fn dial_down(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		self.key_down(instance, settings).await
	}
	async fn dial_up(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
	) -> OpenActionResult<()> {
		self.key_up(instance, settings).await
	}

	async fn dial_rotate(
		&self,
		instance: &Instance,
		settings: &Self::Settings,
		ticks: i16,
		_pressed: bool,
	) -> OpenActionResult<()> {
		let mut clone=settings.clone();
		clone.value=clone.value+ticks;
		if clone.value>255 {
			clone.value=255;
		}
		if clone.value<0 {
			clone.value=0;
		}
		
		send(&Os2l::Cmd { id: settings.channel, param: clone.value.into() }).unwrap_or(());
		instance.set_state(if clone.value>0 { 1 } else { 0 }).await?;
		instance.set_settings(&clone).await?;
		Ok(())
	}
}


#[tokio::main]
async fn main() -> OpenActionResult<()> {
	let _ = std::fs::OpenOptions::new().create(true).append(true).open("/tmp/os2l.log")
	.and_then(|mut f| writeln!(f, "main gestartet"));
	{
		use simplelog::*;
		if let Err(error) = TermLogger::init(
			LevelFilter::Debug,
			Config::default(),
			TerminalMode::Stdout,
			ColorChoice::Never,
		) {
			eprintln!("Logger initialization failed: {}", error);
		}
	}

	register_action(OS2LAction).await;

	run(std::env::args().collect()).await
}
