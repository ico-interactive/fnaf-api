use axum::{
    body::Bytes,
    http::{HeaderMap, StatusCode},
    response::Response,
};
use serenity::{
    Client,
    all::{
        CommandInteraction, CreateInteractionResponse, CreateInteractionResponseMessage,
        Interaction, Message, Verifier,
    },
    async_trait, json,
    prelude::*,
};
use std::{env, error::Error};
use tracing::{error, info, warn};

struct GatewayHandler;

struct DiscordOpts {
    gateway_enabled: bool,
    interaction_enabled: bool,
}

pub struct DiscordHandler {
    config: DiscordOpts,
    verifier: Verifier,
    client: Client,
}

impl DiscordHandler {
    async fn init(&mut self) -> Result<(), Box<dyn Error>> {
        if env::var("GATEWAY_HANDLER").expect("env: no GATEWAY_HANDLER set") == "true" {
            self.config.gateway_enabled = true;
            self.init_gateway().await?;
        }
        if env::var("INTERACTION_HANDLER").expect("env: no INTERACTION_HANDLER set") == "true" {
            self.config.interaction_enabled = true;
            self.init_interactions().await?;
        }
        Ok(())
    }

    async fn init_gateway(&mut self) -> Result<(), Box<dyn Error>> {
        let token = env::var("TOKEN").expect("expected TOKEN to be set in env");
        let intents = GatewayIntents::GUILD_MESSAGES
            | GatewayIntents::DIRECT_MESSAGES
            | GatewayIntents::GUILD_INTEGRATIONS
            | GatewayIntents::MESSAGE_CONTENT;

        // build client
        self.client = Client::builder(&token, intents)
            .event_handler(GatewayHandler)
            .await
            .expect("error creating client");

        // start shard
        if let Err(e) = self.client.start().await {
            error!("client error: {e:?}");
            return Err(e.into());
        }
        Ok(())
    }

    async fn init_interactions(&mut self) -> Result<(), Box<dyn Error>> {
        let public_key = env::var("PUBLIC_KEY").expect("expected PUBLIC_KEY to be set in env");
        self.verifier = Verifier::new(&public_key);
        Ok(())
    }
    async fn handle_interaction(
        self,
        headers: HeaderMap,
        body: Bytes,
    ) -> Response<CreateInteractionResponse> {
        match self.try_handle_interaction(headers, body).await {
            Ok(response) => response,
            Err(e) => {
                warn!("could not handle interaction, error: {}", e);
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header("Content-Type", "application/json")
                    .body(CreateInteractionResponse::Acknowledge)
                    .unwrap()
            }
        }
    }
    async fn try_handle_interaction(
        self,
        headers: HeaderMap,
        body: Bytes,
    ) -> Result<Response<CreateInteractionResponse>, Box<dyn Error>> {
        info!("received interaction from {:?}", headers);
        let signature = headers["X-Signature-Ed25519"]
            .to_str()
            .unwrap_or("invalid signature");
        let timestamp = headers["X-Signature-Timestamp"]
            .to_str()
            .unwrap_or("invalid timestamp");
        let body_data: &[u8] = body.as_ref();
        if let Err(_) = self.verifier.verify(signature, timestamp, body_data) {
            warn!("could not process interaction");
            return Err("could not find handler".into());
        }
        let res_body = match json::from_slice::<Interaction>(body_data).unwrap() {
            Interaction::Ping(_) => CreateInteractionResponse::Pong,
            Interaction::Command(interaction) => self.handle_command(interaction),
            _ => return Err("could not find handler".into()),
        };
        let response = Response::builder()
            .status(200)
            .header("Content-Type", "application/json")
            .body(res_body)
            .unwrap();
        Ok(response)
    }
    fn handle_command(&self, interaction: CommandInteraction) -> CreateInteractionResponse {
        dbg!(&interaction);
        CreateInteractionResponse::Message(CreateInteractionResponseMessage::new().content(
            format!(
                "Hello from interactions webhook HTTP server! <@{}>",
                interaction.user.id
            ),
        ))
    }
}

#[async_trait]
impl EventHandler for GatewayHandler {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.content == "!ping" {
            if let Err(why) = msg.channel_id.say(&ctx.http, "Pong!").await {
                println!("Error sending message: {why:?}");
            }
        }
    }
}
