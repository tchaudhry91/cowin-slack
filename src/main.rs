use chrono::Utc;
use clap::{AppSettings, Clap};
use failure::{err_msg, Error};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

const DUMMY_BROWSER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64; rv:88.0) Gecko/20100101 Firefox/88.0";

const API_BASE: &str = "https://cdn-api.co-vin.in/api/v2/appointment/sessions";

#[derive(Serialize, Deserialize, Debug)]
struct Resp {
    centers: Vec<Center>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Center {
    center_id: i32,
    name: String,
    address: String,
    pincode: i32,
    fee_type: String,
    sessions: Vec<Session>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Session {
    date: String,
    available_capacity: i32,
    min_age_limit: i32,
    vaccine: String,
    available_capacity_dose1: i32,
    available_capacity_dose2: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Slot {
    center: String,
    address: String,
    date: String,
    available_capacity: i32,
    available_capacity_dose1: i32,
    available_capacity_dose2: i32,
    min_age_limit: i32,
    vaccine: String,
}

fn get_today_ist() -> String {
    let date = Utc::now().with_timezone(&chrono_tz::Tz::Asia__Kolkata);
    date.format("%d-%m-%Y").to_string()
}

fn fetch_district_slots(district_id: String) -> Result<Resp, Error> {
    let mut pin_url: String = API_BASE.to_owned();
    pin_url.push_str("/calendarByDistrict?district_id=");
    pin_url.push_str(&district_id);
    pin_url.push_str("&date=");
    pin_url.push_str(&get_today_ist());

    let res = reqwest::blocking::Client::new()
        .get(pin_url)
        .header("User-Agent", DUMMY_BROWSER_AGENT.to_string())
        .header("Pragma", "no-cache")
        .header("Cache-Control", "no-cache")
        .send()?;

    let api_resp: Resp;
    match res.status() {
        StatusCode::OK => {
            api_resp = serde_json::from_str(&res.text()?)?;
            Ok(api_resp)
        }
        s => {
            return Err(err_msg(format!("Bad Return Code: {}", s)));
        }
    }
}

fn check_viable_slots(api_resp: Resp, only_18plus: bool, only_first_dose: bool) -> Vec<Slot> {
    let mut slots: Vec<Slot> = vec![];
    for center in api_resp.centers.iter() {
        for session in center.sessions.iter() {
            if only_18plus && session.min_age_limit > 18 {
                continue;
            }
            if only_first_dose && session.available_capacity_dose1 < 5 {
                continue;
            }
            if session.available_capacity > 0 {
                let slot = Slot {
                    center: center.name.clone(),
                    address: center.address.clone(),
                    date: session.date.clone(),
                    vaccine: session.vaccine.clone(),
                    available_capacity: session.available_capacity,
                    available_capacity_dose1: session.available_capacity_dose1,
                    available_capacity_dose2: session.available_capacity_dose2,
                    min_age_limit: session.min_age_limit,
                };
                slots.push(slot);
            }
        }
    }
    slots
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct SlackPayload {
    channel: String,
    text: String,
    username: String,
}

fn post_slot_to_slack(slot: Slot, hook_url: String, channel: String) -> Result<(), Error> {
    let data_text = format!(
        ":large_green_circle: [Vaccine Slot]
        Date: {},
        Center: {},
        Address: {},
        Vaccine: {},
        Available Capacity: {},
        1st Dose Capacity: {},
        2nd Dose Capacity: {},
        Min Age Limit: {},
        ",
        slot.date,
        slot.center,
        slot.address,
        slot.vaccine,
        slot.available_capacity,
        slot.available_capacity_dose1,
        slot.available_capacity_dose2,
        slot.min_age_limit,
    );

    let payload = SlackPayload {
        text: data_text,
        channel,
        username: String::from("Tux-Sudo CoWin Bot"),
    };

    let client = reqwest::blocking::Client::new();
    client.post(hook_url).json(&payload).send()?;
    Ok(())
}

fn post_debug_to_slack(message: String, hook_url: String, channel: String) -> Result<(), Error> {
    let payload = SlackPayload {
        text: message,
        channel,
        username: String::from("Tux-Sudo CoWin Bot"),
    };
    let client = reqwest::blocking::Client::new();
    client.post(hook_url).json(&payload).send()?;
    Ok(())
}

#[derive(Clap)]
#[clap(
    version = "1.0",
    author = "Tanmay Chaudhry <tanmay.chaudhry@gmail.com>"
)]
#[clap(setting = AppSettings::ColoredHelp)]
struct Opts {
    #[clap(short, long)]
    age_18_plus: bool,

    #[clap(short, long)]
    first_dose_only: bool,

    #[clap(short, long, default_value = "188")]
    district_id: String,

    #[clap(long)]
    slack_hook: String,

    #[clap(long)]
    slack_main_channel: String,

    #[clap(long)]
    slack_debug_channel: String,
}

fn main() {
    let opts: Opts = Opts::parse();

    let api_resp =
        fetch_district_slots(opts.district_id.clone()).expect("Failed to fetch districts.");
    let slots = check_viable_slots(api_resp, opts.age_18_plus, opts.first_dose_only);
    for slot in slots.iter() {
        post_slot_to_slack(
            slot.clone(),
            opts.slack_hook.clone(),
            opts.slack_main_channel.clone(),
        )
        .expect("Failed to post message to slack channel.");
    }
    let output_str = format!(
        "Found {} viable slots for District ID: {}",
        slots.len(),
        opts.district_id
    );
    post_debug_to_slack(
        output_str.clone(),
        opts.slack_hook.clone(),
        opts.slack_debug_channel,
    )
    .expect("Failed to post debug message to slack");
    println!("{}", output_str);
}
