use std::{collections::HashMap, sync::OnceLock};

use chrono::Utc;
use regex_lite::Regex;

use crate::hitomi_client::HitomiClient;

pub struct GG {
    pub last_retrieval: Option<i64>,
    pub m_default: i32,
    pub m_map: HashMap<i32, i32>,
    pub b: String,
}

impl GG {
    fn new() -> Self {
        Self {
            last_retrieval: None,
            m_default: 0,
            m_map: HashMap::new(),
            b: String::new(),
        }
    }

    pub fn inst() -> &'static tokio::sync::Mutex<GG> {
        static INSTANCE: OnceLock<tokio::sync::Mutex<GG>> = OnceLock::new();
        INSTANCE.get_or_init(|| tokio::sync::Mutex::new(GG::new()))
    }

    pub async fn refresh(&mut self) -> anyhow::Result<()> {
        if let Some(last_retrieval) = self.last_retrieval {
            if last_retrieval + 60000 >= Utc::now().timestamp_millis() {
                return Ok(());
            }
        }

        let client = HitomiClient::get_api_client();

        let request = client
            .read()
            .get("https://ltn.gold-usergeneratedcontent.net/gg.js");
        let body = request.send().await?.text().await?;

        let re_default = Regex::new(r"var o = (\d)")?;
        let re_o = Regex::new(r"o = (\d); break;")?;
        let re_case = Regex::new(r"case (\d+):")?;
        let re_b = Regex::new(r"b: '(.+)'")?;

        if let Some(cap) = re_default.captures(&body) {
            self.m_default = cap[1].parse()?;
        }

        if let Some(cap) = re_o.captures(&body) {
            let o: i32 = cap[1].parse()?;
            self.m_map.clear();

            for cap in re_case.captures_iter(&body) {
                let case: i32 = cap[1].parse()?;
                self.m_map.insert(case, o);
            }
        }

        if let Some(cap) = re_b.captures(&body) {
            self.b = cap[1].to_string();
        }

        self.last_retrieval = Some(Utc::now().timestamp_millis());
        Ok(())
    }

    pub async fn m(&mut self, g: i32) -> anyhow::Result<i32> {
        self.refresh().await?;
        Ok(self.m_map.get(&g).copied().unwrap_or(self.m_default))
    }

    pub async fn b(&mut self) -> anyhow::Result<String> {
        self.refresh().await?;
        Ok(self.b.clone())
    }

    #[allow(clippy::unused_self)]
    pub fn s(&self, h: &str) -> anyhow::Result<String> {
        let re = Regex::new(r"(..)(.)$")?;
        if let Some(caps) = re.captures(h) {
            let combined = format!("{}{}", &caps[2], &caps[1]);
            let num = i32::from_str_radix(&combined, 16)?;
            Ok(num.to_string())
        } else {
            Err(anyhow::anyhow!("Invalid hash format"))
        }
    }
}
