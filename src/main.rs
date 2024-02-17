use anyhow::Result;
use chrono::Utc;
use image::{io::Reader as ImageReader, Rgba};
use imageproc::drawing::draw_text_mut;
use regex::Regex;
use rusttype::{Font, Scale};
use serde::{Deserialize, Serialize};
use serenity::{
    builder::{CreateAttachment, EditProfile},
    client::{Client, Context, EventHandler},
    gateway::ActivityData,
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
    pub server_name: Option<String>,
    pub server_id: Option<i64>,
    pub game: Option<String>,
}

/// `MyConfig` implements `Default`
impl ::std::default::Default for Static {
    fn default() -> Self {
        Self {
            token: "".into(),
            server_name: None,
            server_id: None,
            game: Some("bf1".into()),
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
    #[serde(rename = "currentPlayers")]
    pub current_players: i64,
    pub region: String,
    pub country: String,
}

#[serenity::async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, _: Ready) {
        let user = ctx.cache.current_user().clone();
        log::info!("Logged in as {:#?}", user.name);

        let last_update = Arc::new(atomic::AtomicI64::new(0));
        let last_update_clone = Arc::clone(&last_update);

        let cfg: Static = confy::load_path("config.txt").unwrap();

        if let Some(ref server_name) = cfg.server_name {
            log::info!("Started monitoring server with name: {}", server_name);
        } else if let Some(server_id) = cfg.server_id {
            log::info!("Started monitoring server with id: {}", server_id);
        } else {
            log::error!("No server name of id set!");
        }

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
                match status(&ctx, &cfg).await {
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

async fn get(game: &str) -> Result<MarneServerList> {
    let client = reqwest::Client::new();
    let url = match game {
        "bfv" => "https://marne.io/api/v/srvlst/",
        _ => "https://marne.io/api/srvlst/",
    };

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

async fn status(ctx: &Context, statics: &Static) -> Result<()> {
    match get(&statics.game.clone().unwrap_or("bf1".into())).await {
        Ok(status) => {
            let maps = HashMap::from([
                ("MP_Amiens", "Amiens"),
                ("MP_Chateau", "Ballroom Blitz"),
                ("MP_Desert", "Sinai Desert"),
                ("MP_FaoFortress", "Fao Fortress"),
                ("MP_Forest", "Argonne Forest"),
                ("MP_ItalianCoast", "Empire's Edge"),
                ("MP_MountainFort", "Monte Grappa"),
                ("MP_Scar", "St Quentin Scar"),
                ("MP_Suez", "Suez"),
                ("MP_Giant", "Giant's Shadow"),
                ("MP_Fields", "Soissons"),
                ("MP_Graveyard", "Rupture"),
                ("MP_Underworld", "Fort De Vaux"),
                ("MP_Verdun", "Verdun Heights"),
                ("MP_ShovelTown", "Prise de Tahure"),
                ("MP_Trench", "Nivelle Nights"),
                ("MP_Bridge", "Brusilov Keep"),
                ("MP_Islands", "Albion"),
                ("MP_Ravines", "Łupków Pass"),
                ("MP_Tsaritsyn", "Tsaritsyn"),
                ("MP_Valley", "Galicia"),
                ("MP_Volga", "Volga River"),
                ("MP_Beachhead", "Cape Helles"),
                ("MP_Harbor", "Zeebrugge"),
                ("MP_Naval", "Heligoland Bight"),
                ("MP_Ridge", "Achi Baba"),
                ("MP_Alps", "Razor's Edge"),
                ("MP_Blitz", "London Calling"),
                ("MP_Hell", "Passchendaele"),
                ("MP_London", "London Calling: Scourge"),
                ("MP_Offensive", "River Somme"),
                ("MP_River", "Caporetto"),
                // BFV
                ("MP_ArcticFjell", "Fjell 652"),
                ("MP_ArcticFjord", "Narvik"),
                ("MP_Arras", "Arras"),
                ("MP_Devastation", "Devastation"),
                ("MP_Escaut", "twisted steel"),
                ("MP_Foxhunt", "Aerodrome"),
                ("MP_Halfaya", "Hamada"),
                ("MP_Rotterdam", "Rotterdam"),
                ("MP_Hannut", "Panzerstorm"),
                ("MP_Crete", "Mercury"),
                ("MP_Kalamas", "Marita"),
                ("MP_Provence", "Provence"),
                ("MP_SandAndSea", "Al sudan"),
                ("MP_Bunker", "Operation Underground"),
                ("MP_IwoJima", "Iwo jima"),
                ("MP_TropicIslands", "Pacific storm"),
                ("MP_WakeIsland", "Wake island"),
                ("MP_Jungle", "Solomon islands"),
                ("MP_Libya", "Al marj encampment"),
                ("MP_Norway", "lofoten islands"),
                // bfv special maps
                ("DK_Norway", "Halvoy"),
                ("MP_Escaut_US", "Twisted Steel US"),
                ("MP_Hannut_US", "Panzerstorm US"),
                ("MP_GOps_Chapter2_Arras", "Arras (Chapter 2)"),
                ("MP_WE_Fortress_Devastation", "Devastation (Fortress)"),
                ("MP_WE_Fortress_Halfaya", "Hamada (Fortress)"),
                ("MP_WE_Grind_ArcticFjord", "Narvik (Grind)"),
                ("MP_WE_Grind_Devastation", "Devastation (Grind)"),
                ("MP_WE_Grind_Escaut", "Twisted Steel (Grind)"),
                ("MP_WE_Grind_Rotterdam", "Rotterdam (Grind)"),
            ]);

            let images = HashMap::from([
                ("MP_Amiens", "https://cdn.gametools.network/maps/bf1/MP_Amiens_LandscapeLarge-e195589d.jpg"),
                ("MP_Chateau", "https://cdn.gametools.network/maps/bf1/MP_Chateau_LandscapeLarge-244d5987.jpg"),
                ("MP_Desert", "https://cdn.gametools.network/maps/bf1/MP_Desert_LandscapeLarge-d8f749da.jpg"),
                ("MP_FaoFortress", "https://cdn.gametools.network/maps/bf1/MP_FaoFortress_LandscapeLarge-cad1748e.jpg"),
                ("MP_Forest", "https://cdn.gametools.network/maps/bf1/MP_Forest_LandscapeLarge-dfbbe910.jpg"),
                ("MP_ItalianCoast", "https://cdn.gametools.network/maps/bf1/MP_ItalianCoast_LandscapeLarge-1503eec7.jpg"),
                ("MP_MountainFort", "https://cdn.gametools.network/maps/bf1/MP_MountainFort_LandscapeLarge-8a517533.jpg"),
                ("MP_Scar", "https://cdn.gametools.network/maps/bf1/MP_Scar_LandscapeLarge-ee25fbd6.jpg"),
                ("MP_Suez", "https://cdn.gametools.network/maps/bf1/MP_Suez_LandscapeLarge-f630fc76.jpg"),
                ("MP_Giant", "https://cdn.gametools.network/maps/bf1/MP_Giant_LandscapeLarge-dd0b93ef.jpg"),
                ("MP_Fields", "https://cdn.gametools.network/maps/bf1/MP_Fields_LandscapeLarge-5f53ddc4.jpg"),
                ("MP_Graveyard", "https://cdn.gametools.network/maps/bf1/MP_Graveyard_LandscapeLarge-bd1012e6.jpg"),
                ("MP_Underworld", "https://cdn.gametools.network/maps/bf1/MP_Underworld_LandscapeLarge-b6c5c7e7.jpg"),
                ("MP_Verdun", "https://cdn.gametools.network/maps/bf1/MP_Verdun_LandscapeLarge-1a364063.jpg"),
                ("MP_ShovelTown", "https://cdn.gametools.network/maps/bf1/MP_Shoveltown_LandscapeLarge-d0aa5920.jpg"),
                ("MP_Trench", "https://cdn.gametools.network/maps/bf1/MP_Trench_LandscapeLarge-dbd1248f.jpg"),
                ("MP_Bridge", "https://cdn.gametools.network/maps/bf1/MP_Bridge_LandscapeLarge-5b7f1b62.jpg"),
                ("MP_Islands", "https://cdn.gametools.network/maps/bf1/MP_Islands_LandscapeLarge-c9d8272b.jpg"),
                ("MP_Ravines", "https://cdn.gametools.network/maps/bf1/MP_Ravines_LandscapeLarge-1fe0d3f6.jpg"),
                ("MP_Tsaritsyn", "https://cdn.gametools.network/maps/bf1/MP_Tsaritsyn_LandscapeLarge-2dbd3bf5.jpg"),
                ("MP_Valley", "https://cdn.gametools.network/maps/bf1/MP_Valley_LandscapeLarge-8dc1c7ca.jpg"),
                ("MP_Volga", "https://cdn.gametools.network/maps/bf1/MP_Volga_LandscapeLarge-6ac49c25.jpg"),
                ("MP_Beachhead", "https://cdn.gametools.network/maps/bf1/MP_Beachhead_LandscapeLarge-5a13c655.jpg"),
                ("MP_Harbor", "https://cdn.gametools.network/maps/bf1/MP_Harbor_LandscapeLarge-d382c7ea.jpg"),
                ("MP_Naval", "https://cdn.gametools.network/maps/bf1/MP_Naval_LandscapeLarge-dc2e8daf.jpg"),
                ("MP_Ridge", "https://cdn.gametools.network/maps/bf1/MP_Ridge_LandscapeLarge-8c057a19.jpg"),
                ("MP_Alps", "https://cdn.gametools.network/maps/bf1/MP_Alps_LandscapeLarge-7ab30e3e.jpg"),
                ("MP_Blitz", "https://cdn.gametools.network/maps/bf1/MP_Blitz_LandscapeLarge-5e26212f.jpg"),
                ("MP_Hell", "https://cdn.gametools.network/maps/bf1/MP_Hell_LandscapeLarge-7176911c.jpg"),
                ("MP_London", "https://cdn.gametools.network/maps/bf1/MP_London_LandscapeLarge-0b51fe46.jpg"),
                ("MP_Offensive", "https://cdn.gametools.network/maps/bf1/MP_Offensive_LandscapeLarge-6dabdea3.jpg"),
                ("MP_River", "https://cdn.gametools.network/maps/bf1/MP_River_LandscapeLarge-21443ae9.jpg"),
                // bfv
                ("MP_ArcticFjell", "https://cdn.gametools.network/maps/bfv/1080p_MP_ArcticFjell-df3c1290.jpg"),
                ("MP_ArcticFjord", "https://cdn.gametools.network/maps/bfv/1080p_MP_ArcticFjord-7ba29138.jpg"),
                ("MP_Arras", "https://cdn.gametools.network/maps/bfv/1080p_MP_Arras-4b610505.jpg"),
                ("MP_Devastation", "https://cdn.gametools.network/maps/bfv/1080p_MP_Devastation-623dea60.jpg"),
                ("MP_Escaut", "https://cdn.gametools.network/maps/bfv/1080p_MP_Escaut-9764d1fb.jpg"),
                ("MP_Foxhunt", "https://cdn.gametools.network/maps/bfv/1080p_MP_AfricanFox-8ad380a5.jpg"),
                ("MP_Halfaya", "https://cdn.gametools.network/maps/bfv/1080p_MP_AfricanHalfaya-31165f9b.jpg"),
                ("MP_Rotterdam", "https://cdn.gametools.network/maps/bfv/1080p_MP_Rotterdam-55632240.jpg"),
                ("MP_Hannut", "https://cdn.gametools.network/maps/bfv/1080p_MP_Hannut-ebbe7197.jpg"),
                ("MP_Crete", "https://cdn.gametools.network/maps/bfv/1080p_MP_Crete-304a202d.jpg"),
                ("MP_Kalamas", "https://cdn.gametools.network/maps/bfv/1080p_MP_Kalamas-c64c8451.jpg"),
                ("MP_Provence", "https://cdn.gametools.network/maps/bfv/1080p_MP_ProvenceXL-a950ad3e.jpg"),
                ("MP_SandAndSea", "https://cdn.gametools.network/maps/bfv/1080p_MP_SandAndSea-f071e6f7.jpg"),
                ("MP_Bunker", "https://cdn.gametools.network/maps/bfv/1080p_MP_Bunker-7b518876.jpg"),
                ("MP_IwoJima", "https://cdn.gametools.network/maps/bfv/1080p_MP_IwoJima-760850fc.jpg"),
                ("MP_TropicIslands", "https://cdn.gametools.network/maps/bfv/1080p_MP_TropicIslands-9e0a41c3.jpg"),
                ("MP_WakeIsland", "https://cdn.gametools.network/maps/bfv/1080p_MP_WakeIsland-3238b455.jpg"),
                ("MP_Jungle", "https://cdn.gametools.network/maps/bfv/1080p_MP_Jungle-714218ce.jpg"),
                ("MP_Libya", "https://cdn.gametools.network/maps/bfv/1080p_MP_Libya-bd54b090.jpg"),
                ("MP_Norway", "https://cdn.gametools.network/maps/bfv/1080p_MP_Norway-7d6d6300.jpg"),
                // bfv special maps
                ("DK_Norway", "https://cdn.gametools.network/maps/bfv/1080p_MP_Norway-7d6d6300.jpg"),
                ("MP_Escaut_US", "https://cdn.gametools.network/maps/bfv/1080p_MP_Escaut-9764d1fb.jpg"),
                ("MP_Hannut_US", "https://cdn.gametools.network/maps/bfv/1080p_MP_Hannut-ebbe7197.jpg"),
                ("MP_GOps_Chapter2_Arras", "https://cdn.gametools.network/maps/bfv/1080p_MP_Arras-4b610505.jpg"),
                ("MP_WE_Fortress_Devastation", "https://cdn.gametools.network/maps/bfv/1080p_MP_Devastation-623dea60.jpg"),
                ("MP_WE_Fortress_Halfaya", "https://cdn.gametools.network/maps/bfv/1080p_MP_AfricanHalfaya-31165f9b.jpg"),
                ("MP_WE_Grind_ArcticFjord", "https://cdn.gametools.network/maps/bfv/1080p_MP_ArcticFjord-7ba29138.jpg"),
                ("MP_WE_Grind_Devastation", "https://cdn.gametools.network/maps/bfv/1080p_MP_Devastation-623dea60.jpg"),
                ("MP_WE_Grind_Escaut", "https://cdn.gametools.network/maps/bfv/1080p_MP_Escaut-9764d1fb.jpg"),
                ("MP_WE_Grind_Rotterdam", "https://cdn.gametools.network/maps/bfv/1080p_MP_Rotterdam-55632240.jpg"),
            ]);

            let small_modes = HashMap::from([
                ("Conquest0", "CQ"),
                ("Rush0", "RS"),
                ("BreakThrough0", "SO"),
                ("BreakthroughLarge0", "OP"),
                ("Possession0", "WP"),
                ("TugOfWar0", "FL"),
                ("AirAssault0", "AA"),
                ("Domination0", "DM"),
                ("TeamDeathMatch0", "TM"),
                ("ZoneControl0", "RS"),
            ]);

            for server in status.servers {
                let right_server = match &statics.server_name {
                    Some(server_name) => &server.name == server_name,
                    None => match &statics.server_id {
                        Some(server_id) => &server.id == server_id,
                        None => false,
                    },
                };

                if right_server {
                    let internal_map =
                        match Regex::new(r"[^\/]+$").unwrap().find(&server.map_name[..]) {
                            Some(location) => location.as_str(),
                            None => &server.map_name[..],
                        };

                    let server_info = format!(
                        "{}/{} - {}",
                        server.current_players,
                        server.max_players,
                        maps.get(internal_map).unwrap_or(&internal_map)
                    );
                    // change game activity
                    ctx.set_activity(Some(ActivityData::playing(server_info)));

                    let image_loc = gen_img(
                        small_modes.get(&server.game_mode[..]).unwrap_or(&""),
                        images.get(internal_map).unwrap_or(&internal_map),
                    )
                    .await?;

                    // change avatar
                    let avatar = CreateAttachment::path(image_loc)
                        .await
                        .expect("Failed to read image");
                    let mut user = ctx.cache.current_user().clone();
                    let _ = user.edit(ctx, EditProfile::new().avatar(&avatar)).await;

                    return Ok(());
                }
            }
        }
        Err(e) => {
            let server_info = "¯\\_(ツ)_/¯ server not found";
            ctx.set_activity(Some(ActivityData::playing(server_info)));

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
                server_name: None,
                server_id: Some(0),
                game: Some("bf1".into()),
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
