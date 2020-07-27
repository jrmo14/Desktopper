use crate::frontend::buttons::RELEASED;
use crate::frontend::{Buttons, Screen};
use gpio_lcd::scheduler::{Job, ThreadedLcd};
use rspotify::blocking::client::Spotify;
use rspotify::blocking::oauth2::{SpotifyClientCredentials, SpotifyOAuth};
use rspotify::blocking::util::get_token;
use rspotify::model::device::Device;
use rspotify::model::offset::Offset;
use rspotify::model::PlayingItem;
use rspotify::senum::AdditionalType;
use std::time::Instant;
use tokio::time::Duration;

pub struct SpotifyScreen {
    oauth: SpotifyOAuth,
    client: Spotify,
    cur_item: Option<PlayingItem>,
    item_duration: u32,
    last_refresh: Instant,
    state: Option<State>,
}

struct State {
    ctx_uri: Option<String>,
    uris: Option<Vec<String>>,
    offset: Option<Offset>,
    pos: Option<u32>,
}
// TODO allow to load as service -- fix authentication somehow, maybe post to page?
// TODO refresh tokens when/if needed
impl SpotifyScreen {
    pub fn new(id: &str, secret: &str, redirect: &str) -> Result<Self, &'static str> {
        let mut oauth = SpotifyOAuth::default()
            .scope(
                "user-modify-playback-state user-read-playback-state user-read-currently-playing",
            )
            .client_id(id)
            .client_secret(secret)
            .redirect_uri(redirect)
            .build();

        match get_token(&mut oauth) {
            Some(token_info) => {
                let creds = SpotifyClientCredentials::default()
                    .token_info(token_info)
                    .build();
                let client = Spotify::default().client_credentials_manager(creds).build();
                Ok(SpotifyScreen {
                    oauth,
                    client,
                    cur_item: None,
                    item_duration: 0,
                    last_refresh: Instant::now(),
                    state: None,
                })
            }
            None => Err("Failed to create Spotify client"),
        }
    }

    fn update_display(&mut self, lcd: &mut ThreadedLcd, force: bool) {
        let result = self
            .client
            .current_playing(None, Some(vec![AdditionalType::Episode]));
        match result {
            Ok(ctx) => match ctx {
                Some(currently_playing) => {
                    let fmt_ms = |ms: u32| -> String {
                        let secs = ms / 1000;
                        let hours = secs / (3600);
                        let mins = (secs - (hours * 3600)) / 60;
                        let secs = secs - (hours * 3600) - (mins * 60);
                        if hours == 0 {
                            format!("{:02}:{:02}", mins, secs)
                        } else {
                            format!("{:02}:{:02}:{:02}", hours, mins, secs)
                        }
                    };

                    let extract_id = |item: &PlayingItem| -> String {
                        match item {
                            PlayingItem::Episode(e) => e.id.clone(),
                            PlayingItem::Track(t) => {
                                t.id.as_ref().unwrap_or(&"".to_string()).to_string()
                            }
                        }
                    };

                    let stale_item = (currently_playing.item.is_none() && self.cur_item.is_none())
                        || (currently_playing
                            .item
                            .as_ref()
                            .map_or("".to_string(), extract_id)
                            == self.cur_item.as_ref().map_or("".to_string(), extract_id));
                    if !stale_item || force {
                        match currently_playing.item.as_ref().unwrap() {
                            PlayingItem::Track(track) => {
                                let top = format!(
                                    "{} -- {}",
                                    track
                                        .artists
                                        .iter()
                                        .map(|artist| artist.name.clone())
                                        .collect::<Vec<String>>()
                                        .join(", "),
                                    track.name
                                );
                                lcd.clear_row(0);
                                lcd.add_job(Job::new(
                                    top.as_str(),
                                    0,
                                    Some(Duration::from_millis(250)),
                                ));
                                self.item_duration = track.duration_ms;
                            }
                            PlayingItem::Episode(episode) => {
                                let top = format!(
                                    "{} {} by {}",
                                    if currently_playing.is_playing {
                                        "Listening to"
                                    } else {
                                        "Paused"
                                    },
                                    episode.name,
                                    episode.show.name
                                );
                                lcd.clear_row(0);
                                lcd.add_job(Job::new(
                                    top.as_str(),
                                    0,
                                    Some(Duration::from_millis(250)),
                                ));
                                self.item_duration = episode.duration_ms;
                            }
                        }
                        self.cur_item = currently_playing.item;
                    }
                    let bottom = format!(
                        "{}/{}",
                        fmt_ms(currently_playing.progress_ms.unwrap_or(0)),
                        fmt_ms(self.item_duration)
                    );
                    lcd.clear_row(1);
                    lcd.add_job(Job::new(
                        bottom.as_str(),
                        1,
                        Some(Duration::from_millis(250)),
                    ));
                }
                None => {
                    lcd.clear_jobs();
                    lcd.add_job(Job::new("Nothings playing", 0, None));
                    lcd.add_job(Job::empty(1));
                }
            },
            Err(e) => {
                lcd.clear_jobs();
                lcd.add_job(Job::new(
                    "Error getting status, might need to update token, consider restart",
                    0,
                    Option::from(Duration::from_millis(250)),
                ));
                lcd.add_job(Job::empty(1));
            }
        };
    }

    fn get_cur_device(&self) -> Option<String> {
        match self.client.device() {
            Ok(devices) => {
                match devices
                    .devices
                    .into_iter()
                    .filter(|device| device.is_active)
                    .map(|device| device.id)
                    .collect::<Vec<String>>()
                    .get(0)
                {
                    Some(id) => Some(id.clone()),
                    None => None,
                }
            }
            Err(_) => None,
        }
    }
}

impl Screen for SpotifyScreen {
    fn first_load(&mut self, lcd: &mut ThreadedLcd) {
        self.update_display(lcd, true);
    }

    fn update_screen(&mut self, lcd: &mut ThreadedLcd, buttons: Buttons) {
        let cur_dev_id = self.get_cur_device();
        if buttons.f0.state == RELEASED {
            let result = self.client.previous_track(cur_dev_id);
            match result {
                Ok(_) => {}
                Err(e) => error!("{}", e),
            }
        } else if buttons.f1.state == RELEASED {
            let playing = match self
                .client
                .current_playing(None, Some(vec![AdditionalType::Episode]))
            {
                Ok(ctx) => match ctx {
                    Some(ctx) => ctx.is_playing,
                    None => {
                        error!("Failed to unwrap context");
                        false
                    }
                },
                Err(e) => {
                    error!("Failed to unwrap context: {}", e);
                    false
                }
            };
            if playing {
                let ctx = self.client.current_playing(None, None).unwrap().unwrap();
                self.state = Some(State {
                    ctx_uri: Option::from(ctx.context.unwrap().uri),
                    uris: None,
                    offset: None,
                    pos: ctx.progress_ms,
                });
                let result = self.client.pause_playback(cur_dev_id);
                match result {
                    Ok(_) => {}
                    Err(e) => error!("{}", e),
                }
            } else {
                let result = self
                    .client
                    .start_playback(cur_dev_id, None, None, None, None);
                match result {
                    Ok(_) => {}
                    Err(e) => error!("{}", e),
                }
            }
        } else if buttons.f2.state == RELEASED {
            let result = self.client.next_track(cur_dev_id);
            match result {
                Ok(_) => {}
                Err(e) => error!("{}", e),
            }
        }
        self.update_display(lcd, true);
    }

    fn get_tick(&self) -> Option<Duration> {
        Some(Duration::from_millis(200))
    }

    // This is the base action for this screen
    fn tick(&mut self, lcd: &mut ThreadedLcd) {
        self.update_display(lcd, false);
    }

    fn get_name(&self) -> String {
        "Music".to_string()
    }
}
