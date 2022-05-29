use std::sync::Arc;

use couchbase::{Cluster, QueryOptions};
use dotenv::dotenv;
use futures::StreamExt;
use lib::LeakData;
use log::error;
use teloxide::{
    dispatching::{DpHandlerDescription, UpdateFilterExt},
    prelude::*,
    utils::command::BotCommands,
    utils::markdown,
};
use tokio::sync::Mutex;

mod config;
use crate::config::CONFIG;

#[derive(BotCommands, Clone)]
#[command(rename = "lowercase", description = "These commands are supported:")]
enum Command {
    #[command(description = "display this text.")]
    Help,
    #[command(description = "Find leaks with domain")]
    Domain(String),
}

type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

async fn handle_domain(
    bot: &AutoSend<Bot>,
    msg: &Message,
    app_data: &AppData,
    domain: &str,
) -> HandlerResult {
    let params = [domain];
    let options = QueryOptions::default().positional_parameters(params);

    let query = format!(
        "SELECT domain, credentials FROM {}:`{}`.`{}`.`{}` WHERE domain = $1 LIMIT 1",
        CONFIG.couch_namespace, CONFIG.couch_bucket, CONFIG.couch_scope, CONFIG.couch_collection
    );

    let mut res = match app_data.cluster.query(query, options).await {
        Ok(res) => res,
        Err(e) => {
            error! {"{:#?}", e};
            return Err(Box::new(e));
        }
    };
    let _md = res.meta_data().await;
    let mut rows = res.rows::<LeakData>();
    let mut rtn_msg = String::new();

    while let Some(leak_data) = rows.next().await {
        let leak_data = leak_data?;
        let creds: Vec<String> = leak_data
            .credentials
            .into_iter()
            .flat_map(|x| {
                x.data
                    .into_iter()
                    .map(|(username, password)| format!("{}:{}", username, password))
            })
            .collect();
        let fmt_str = creds.join("\n");
        rtn_msg.push_str(&fmt_str);
    }

    if rtn_msg.is_empty() {
        bot.send_message(msg.chat.id, "Nothing found :(").await?;
    } else if rtn_msg.len() > 5000 {
        bot.send_message(msg.chat.id, "To much data for tg. WIP")
            .await?;
    } else {
        let rtn_msg = markdown::code_block(rtn_msg.trim_end());
        bot.parse_mode(teloxide::types::ParseMode::MarkdownV2)
            .send_message(msg.chat.id, rtn_msg)
            .await?;
    }

    Ok(())
}

async fn handle_command(
    bot: AutoSend<Bot>,
    msg: Message,
    cmd: Command,
    app_data: Arc<Mutex<AppData>>,
) -> HandlerResult {
    match cmd {
        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string())
                .await?;
        }
        Command::Domain(domain) => {
            let app_data = app_data.lock().await;
            handle_domain(&bot, &msg, &*app_data, &domain).await?;
        }
    }
    Ok(())
}

fn schema() -> Handler<'static, DependencyMap, HandlerResult, DpHandlerDescription> {
    Update::filter_message().branch(
        dptree::entry()
            .filter_command::<Command>()
            .endpoint(handle_command),
    )
}

struct AppData {
    pub cluster: Cluster,
}

async fn init_db() -> Result<Cluster, Box<dyn std::error::Error + Send + Sync>> {
    let cluster = Cluster::connect(
        &CONFIG.couch_uri,
        &CONFIG.couch_username,
        &CONFIG.couch_password,
    );

    Ok(cluster)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenv().ok();
    env_logger::init();
    log::info!("Starting command bot...");

    let cluster = init_db().await?;

    let app_data = AppData { cluster };
    let app_data = Arc::new(Mutex::new(app_data));

    let bot = Bot::from_env().auto_send();
    Dispatcher::builder(bot, schema())
        .dependencies(dptree::deps![app_data])
        .build()
        .setup_ctrlc_handler()
        .dispatch()
        .await;
    Ok(())
}
