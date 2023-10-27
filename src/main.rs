use anyhow::Result;
use chrono::Utc;
use image::{io::Reader as ImageReader, Rgba};
use imageproc::drawing::draw_text_mut;
use rusttype::{Font, Scale};
use serde::{Deserialize, Serialize};
use serenity::{
    client::{Client, Context, EventHandler},
    model::gateway::Activity,
    model::gateway::Ready,
    prelude::GatewayIntents,
};
use std::{collections::HashMap, io::Cursor};
use std::{
    sync::{atomic, Arc},
    time,
};
use warp::Filter;

struct Handler;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Static {
    pub token: String,
    pub server_name: String,
}

/// `MyConfig` implements `Default`
impl ::std::default::Default for Static {
    fn default() -> Self {
        Self {
            token: "".into(),
            server_name: "".into(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Player {
    pub name: String,
    pub team: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Mod {
    pub category: String,
    pub file_name: String,
    pub link: String,
    pub name: String,
    pub version: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum ModType {
    Vec(Vec<Mod>),
    String(String),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum PlayerType {
    Vec(Vec<Player>),
    String(String),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MarneServerList {
    pub servers: Vec<MarneServerInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MarneServerInfo {
    pub id: i64,
    pub name: String,
    #[serde(rename = "mapName")]
    pub map_name: String,
    #[serde(rename = "gameMode")]
    pub game_mode: String,
    #[serde(rename = "maxPlayers")]
    pub max_players: i64,
    #[serde(rename = "tickRate")]
    pub tick_rate: i64,
    pub password: i64,
    #[serde(rename = "needSameMods")]
    pub need_same_mods: i64,
    #[serde(rename = "allowMoreMods")]
    pub allow_more_mods: i64,
    #[serde(rename = "modList")]
    pub mod_list: ModType,
    #[serde(rename = "playerList")]
    pub player_list: PlayerType,
    #[serde(rename = "currentPlayers")]
    pub current_players: i64,
    pub region: String,
    pub country: String,
}

#[serenity::async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, _: Ready) {
        let user = ctx.cache.current_user();
        log::info!("Logged in as {:#?}", user.name);

        let last_update = Arc::new(atomic::AtomicI64::new(0));
        let last_update_clone = Arc::clone(&last_update);

        let cfg: Static = confy::load_path("config.txt").unwrap();

        log::info!("Started monitoring server {}", cfg.server_name);

        tokio::spawn(async move {
            let hello = warp::any().map(move || {
                let last_update_i64 = last_update_clone.load(atomic::Ordering::Relaxed);
                let now_minutes = Utc::now().timestamp() / 60;
                if (now_minutes - last_update_i64) > 5 {
                    warp::reply::with_status(
                        format!("{}", now_minutes - last_update_i64),
                        warp::http::StatusCode::SERVICE_UNAVAILABLE,
                    )
                } else {
                    warp::reply::with_status(
                        format!("{}", now_minutes - last_update_i64),
                        warp::http::StatusCode::OK,
                    )
                }
            });
            warp::serve(hello).run(([0, 0, 0, 0], 3030)).await;
        });

        // loop in seperate async
        tokio::spawn(async move {
            loop {
                match status(ctx.clone(), cfg.clone()).await {
                    Ok(item) => item,
                    Err(e) => {
                        log::error!("cant get new stats: {}", e);
                    }
                };
                last_update.store(Utc::now().timestamp() / 60, atomic::Ordering::Relaxed);
                // wait 2 minutes before redo
                tokio::time::sleep(time::Duration::from_secs(60)).await;
            }
        });
    }
}

async fn get() -> Result<MarneServerList> {
    let client = reqwest::Client::new();
    let url = "https://marne.io/api/srvlst/";

    match client.get(url).send().await {
        Ok(resp) => {
            let mut json_string = resp.text().await.unwrap_or_default();
            // remove weird 0 width character
            // https://github.com/seanmonstar/reqwest/issues/426
            let json_bytes = json_string.as_bytes();
            if json_bytes[0] == 239 {
                json_string.remove(0);
            }
            match serde_json::from_str::<MarneServerList>(&json_string) {
                Ok(json_res) => Ok(json_res),
                Err(e) => {
                    anyhow::bail!("marne public json is incorrect: {:#?}", e)
                }
            }
        }
        Err(e) => {
            anyhow::bail!("marne public url failed: {:#?}", e)
        }
    }
}

async fn status(ctx: Context, statics: Static) -> Result<()> {
    match get().await {
        Ok(status) => {
            for server in status.servers {
                if server.name == statics.server_name {
                    let maps = HashMap::from([
                        ("Levels/MP/MP_Amiens/MP_Amiens", "Amiens"),
                        ("Levels/MP/MP_Chateau/MP_Chateau", "Ballroom Blitz"),
                        ("Levels/MP/MP_Desert/MP_Desert", "Sinai Desert"),
                        ("Levels/MP/MP_FaoFortress/MP_FaoFortress", "Fao Fortress"),
                        ("Levels/MP/MP_Forest/MP_Forest", "Argonne Forest"),
                        ("Levels/MP/MP_ItalianCoast/MP_ItalianCoast", "Empire's Edge"),
                        ("Levels/MP/MP_MountainFort/MP_MountainFort", "Monte Grappa"),
                        ("Levels/MP/MP_Scar/MP_Scar", "St Quentin Scar"),
                        ("Levels/MP/MP_Suez/MP_Suez", "Suez"),
                        ("Xpack0/Levels/MP/MP_Giant/MP_Giant", "Giant's Shadow"),
                        ("Xpack1/Levels/MP_Fields/MP_Fields", "Soissons"),
                        ("Xpack1/Levels/MP_Graveyard/MP_Graveyard", "Rupture"),
                        ("Xpack1/Levels/MP_Underworld/MP_Underworld", "Fort De Vaux"),
                        ("Xpack1/Levels/MP_Verdun/MP_Verdun", "Verdun Heights"),
                        (
                            "Xpack1-3/Levels/MP_ShovelTown/MP_ShovelTown",
                            "Prise de Tahure",
                        ),
                        ("Xpack1-3/Levels/MP_Trench/MP_Trench", "Nivelle Nights"),
                        ("Xpack2/Levels/MP/MP_Bridge/MP_Bridge", "Brusilov Keep"),
                        ("Xpack2/Levels/MP/MP_Islands/MP_Islands", "Albion"),
                        ("Xpack2/Levels/MP/MP_Ravines/MP_Ravines", "Łupków Pass"),
                        ("Xpack2/Levels/MP/MP_Tsaritsyn/MP_Tsaritsyn", "Tsaritsyn"),
                        ("Xpack2/Levels/MP/MP_Valley/MP_Valley", "Galicia"),
                        ("Xpack2/Levels/MP/MP_Volga/MP_Volga", "Volga River"),
                        ("Xpack3/Levels/MP/MP_Beachhead/MP_Beachhead", "Cape Helles"),
                        ("Xpack3/Levels/MP/MP_Harbor/MP_Harbor", "Zeebrugge"),
                        ("Xpack3/Levels/MP/MP_Naval/MP_Naval", "Heligoland Bight"),
                        ("Xpack3/Levels/MP/MP_Ridge/MP_Ridge", "Achi Baba"),
                        ("Xpack4/Levels/MP/MP_Alps/MP_Alps", "Razor's Edge"),
                        ("Xpack4/Levels/MP/MP_Blitz/MP_Blitz", "London Calling"),
                        ("Xpack4/Levels/MP/MP_Hell/MP_Hell", "Passchendaele"),
                        (
                            "Xpack4/Levels/MP/MP_London/MP_London",
                            "London Calling: Scourge",
                        ),
                        ("Xpack4/Levels/MP/MP_Offensive/MP_Offensive", "River Somme"),
                        ("Xpack4/Levels/MP/MP_River/MP_River", "Caporetto"),
                    ]);

                    let images = HashMap::from([
                        ("Levels/MP/MP_Amiens/MP_Amiens", "https://cdn.gametools.network/maps/bf1/MP_Amiens_LandscapeLarge-e195589d.jpg"),
                        ("Levels/MP/MP_Chateau/MP_Chateau", "https://cdn.gametools.network/maps/bf1/MP_Chateau_LandscapeLarge-244d5987.jpg"),
                        ("Levels/MP/MP_Desert/MP_Desert", "https://cdn.gametools.network/maps/bf1/MP_Desert_LandscapeLarge-d8f749da.jpg"),
                        ("Levels/MP/MP_FaoFortress/MP_FaoFortress", "https://cdn.gametools.network/maps/bf1/MP_FaoFortress_LandscapeLarge-cad1748e.jpg"),
                        ("Levels/MP/MP_Forest/MP_Forest", "https://cdn.gametools.network/maps/bf1/MP_Forest_LandscapeLarge-dfbbe910.jpg"),
                        ("Levels/MP/MP_ItalianCoast/MP_ItalianCoast", "https://cdn.gametools.network/maps/bf1/MP_ItalianCoast_LandscapeLarge-1503eec7.jpg"),
                        ("Levels/MP/MP_MountainFort/MP_MountainFort", "https://cdn.gametools.network/maps/bf1/MP_MountainFort_LandscapeLarge-8a517533.jpg"),
                        ("Levels/MP/MP_Scar/MP_Scar", "https://cdn.gametools.network/maps/bf1/MP_Scar_LandscapeLarge-ee25fbd6.jpg"),
                        ("Levels/MP/MP_Suez/MP_Suez", "https://cdn.gametools.network/maps/bf1/MP_Suez_LandscapeLarge-f630fc76.jpg"),
                        ("Xpack0/Levels/MP/MP_Giant/MP_Giant", "https://cdn.gametools.network/maps/bf1/MP_Giant_LandscapeLarge-dd0b93ef.jpg"),
                        ("Xpack1/Levels/MP_Fields/MP_Fields", "https://cdn.gametools.network/maps/bf1/MP_Fields_LandscapeLarge-5f53ddc4.jpg"),
                        ("Xpack1/Levels/MP_Graveyard/MP_Graveyard", "https://cdn.gametools.network/maps/bf1/MP_Graveyard_LandscapeLarge-bd1012e6.jpg"),
                        ("Xpack1/Levels/MP_Underworld/MP_Underworld", "https://cdn.gametools.network/maps/bf1/MP_Underworld_LandscapeLarge-b6c5c7e7.jpg"),
                        ("Xpack1/Levels/MP_Verdun/MP_Verdun", "https://cdn.gametools.network/maps/bf1/MP_Verdun_LandscapeLarge-1a364063.jpg"),
                        ("Xpack1-3/Levels/MP_ShovelTown/MP_ShovelTown", "https://cdn.gametools.network/maps/bf1/MP_Shoveltown_LandscapeLarge-d0aa5920.jpg"),
                        ("Xpack1-3/Levels/MP_Trench/MP_Trench", "https://cdn.gametools.network/maps/bf1/MP_Trench_LandscapeLarge-dbd1248f.jpg"),
                        ("Xpack2/Levels/MP/MP_Bridge/MP_Bridge", "https://cdn.gametools.network/maps/bf1/MP_Bridge_LandscapeLarge-5b7f1b62.jpg"),
                        ("Xpack2/Levels/MP/MP_Islands/MP_Islands", "https://cdn.gametools.network/maps/bf1/MP_Islands_LandscapeLarge-c9d8272b.jpg"),
                        ("Xpack2/Levels/MP/MP_Ravines/MP_Ravines", "https://cdn.gametools.network/maps/bf1/MP_Ravines_LandscapeLarge-1fe0d3f6.jpg"),
                        ("Xpack2/Levels/MP/MP_Tsaritsyn/MP_Tsaritsyn", "https://cdn.gametools.network/maps/bf1/MP_Tsaritsyn_LandscapeLarge-2dbd3bf5.jpg"),
                        ("Xpack2/Levels/MP/MP_Valley/MP_Valley", "https://cdn.gametools.network/maps/bf1/MP_Valley_LandscapeLarge-8dc1c7ca.jpg"),
                        ("Xpack2/Levels/MP/MP_Volga/MP_Volga", "https://cdn.gametools.network/maps/bf1/MP_Volga_LandscapeLarge-6ac49c25.jpg"),
                        ("Xpack3/Levels/MP/MP_Beachhead/MP_Beachhead", "https://cdn.gametools.network/maps/bf1/MP_Beachhead_LandscapeLarge-5a13c655.jpg"),
                        ("Xpack3/Levels/MP/MP_Harbor/MP_Harbor", "https://cdn.gametools.network/maps/bf1/MP_Harbor_LandscapeLarge-d382c7ea.jpg"),
                        ("Xpack3/Levels/MP/MP_Naval/MP_Naval", "https://cdn.gametools.network/maps/bf1/MP_Naval_LandscapeLarge-dc2e8daf.jpg"),
                        ("Xpack3/Levels/MP/MP_Ridge/MP_Ridge", "https://cdn.gametools.network/maps/bf1/MP_Ridge_LandscapeLarge-8c057a19.jpg"),
                        ("Xpack4/Levels/MP/MP_Alps/MP_Alps", "https://cdn.gametools.network/maps/bf1/MP_Alps_LandscapeLarge-7ab30e3e.jpg"),
                        ("Xpack4/Levels/MP/MP_Blitz/MP_Blitz", "https://cdn.gametools.network/maps/bf1/MP_Blitz_LandscapeLarge-5e26212f.jpg"),
                        ("Xpack4/Levels/MP/MP_Hell/MP_Hell", "https://cdn.gametools.network/maps/bf1/MP_Hell_LandscapeLarge-7176911c.jpg"),
                        ("Xpack4/Levels/MP/MP_London/MP_London", "https://cdn.gametools.network/maps/bf1/MP_London_LandscapeLarge-0b51fe46.jpg"),
                        ("Xpack4/Levels/MP/MP_Offensive/MP_Offensive", "https://cdn.gametools.network/maps/bf1/MP_Offensive_LandscapeLarge-6dabdea3.jpg"),
                        ("Xpack4/Levels/MP/MP_River/MP_River", "https://cdn.gametools.network/maps/bf1/MP_River_LandscapeLarge-21443ae9.jpg"),
                    ]);

                    let small_modes = HashMap::from([
                        ("Conquest0", "CQ"),
                        ("Rush0", "RS"),
                        ("BreakThrough0", "SO"),
                        ("BreakThroughLarge0", "OP"),
                        ("Possession0", "WP"),
                        ("TugOfWar0", "FL"),
                        ("AirAssault0", "AA"),
                        ("Domination0", "DM"),
                        ("TeamDeathMatch0", "TM"),
                        ("ZoneControl0", "RS"),
                    ]);

                    let server_info = format!(
                        "{}/{} - {}",
                        server.current_players,
                        server.max_players,
                        maps.get(&server.map_name[..])
                            .unwrap_or(&&server.map_name[..])
                    );
                    // change game activity
                    ctx.set_activity(Activity::playing(server_info)).await;

                    let image_loc = gen_img(
                        small_modes.get(&server.game_mode[..]).unwrap_or(&""),
                        images
                            .get(&server.map_name[..])
                            .unwrap_or(&&server.map_name[..]),
                    )
                    .await?;

                    // change avatar
                    let avatar =
                        serenity::utils::read_image(image_loc).expect("Failed to read image");
                    let mut user = ctx.cache.current_user();
                    let _ = user.edit(&ctx, |p| p.avatar(Some(&avatar))).await;

                    return Ok(());
                }
            }
        }
        Err(e) => {
            let server_info = "¯\\_(ツ)_/¯ server not found";
            ctx.set_activity(Activity::playing(server_info)).await;

            anyhow::bail!(format!("Failed to get new serverinfo: {}", e))
        }
    };
    anyhow::bail!(format!("Couldn't find server in serverlist!"))
}

pub async fn gen_img(small_mode: &str, map_image: &str) -> Result<String> {
    let client = reqwest::Client::new();
    let img = client.get(map_image).send().await?.bytes().await?;

    let mut img2 = ImageReader::new(Cursor::new(img))
        .with_guessed_format()?
        .decode()?
        .brighten(-25);

    img2.save("./info_image.jpg")?;

    let scale = Scale {
        x: (img2.width() / 3) as f32,
        y: (img2.height() as f32 / 1.7),
    };
    let font_name = Vec::from(include_bytes!("Futura.ttf") as &[u8]);
    let font: Font = Font::try_from_vec(font_name).unwrap();

    let img_size = Scale {
        x: img2.width() as f32,
        y: img2.height() as f32,
    };

    draw_text_mut(
        &mut img2,
        Rgba([255u8, 255u8, 255u8, 255u8]),
        (img_size.x / 3.5) as i32,
        (img_size.y / 4.8) as i32,
        scale,
        &font,
        small_mode,
    );
    img2.save("./map_mode.jpg")?;

    Ok(String::from("./map_mode.jpg"))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    log::set_max_level(log::LevelFilter::Info);
    flexi_logger::Logger::try_with_str("warn,discord_bot=info")
        .unwrap_or_else(|e| panic!("Logger initialization failed with {}", e))
        .start()?;

    let cfg: Static = match confy::load_path("config.txt") {
        Ok(config) => config,
        Err(e) => {
            log::error!("error in config.txt: {}", e);
            log::warn!("changing back to default..");
            Static {
                token: "".into(),
                server_name: "".into(),
            }
        }
    };
    confy::store_path("config.txt", cfg.clone()).unwrap();

    // Login with a bot token from the environment
    let intents = GatewayIntents::non_privileged();
    let mut client = Client::builder(cfg.token, intents)
        .event_handler(Handler)
        .await
        .expect("Error creating client");

    // start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        log::error!("Client error: {:?}", why);
    }
    Ok(())
}
