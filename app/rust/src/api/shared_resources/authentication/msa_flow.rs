use std::ops::Add;

use anyhow::Context;
use chrono::{Duration, Utc};
use oauth2::basic::{BasicClient, BasicErrorResponseType, BasicTokenType};
use oauth2::reqwest::async_http_client;
use oauth2::{
    AuthUrl, ClientId, DeviceAuthorizationUrl, EmptyExtraTokenFields, RevocationErrorResponseType,
    Scope, StandardDeviceAuthorizationResponse, StandardErrorResponse, StandardRevocableToken,
    StandardTokenIntrospectionResponse, StandardTokenResponse, TokenResponse, TokenUrl,
};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::sleep;
use uuid::Uuid;

use super::account::{AccountToken, MinecraftAccount, MinecraftCape, MinecraftSkin};
use super::{
    MINECRAFT_AUTH_URL, MINECRAFT_USER_PROFILE_URL, MSA_CLIENT_ID, MSA_DEVICE_CODE_URL, MSA_SCOPE,
    MSA_TOKEN_URL, MSA_URL, XBOX_LIVE_AUTH_URL, XBOX_LIVE_XSTS_URL,
};

#[derive(Debug, Clone)]
pub enum LoginFlowEvent {
    Stage(LoginFlowStage),
    DeviceCode(LoginFlowDeviceCode),
    Success(MinecraftAccount),
}

#[derive(Debug, Clone)]
pub struct LoginFlowDeviceCode {
    pub verification_uri: String,
    pub user_code: String,
}

#[derive(Debug, Clone)]
pub enum LoginFlowStage {
    FetchingDeviceCode,
    WaitingForUser,
    AuthenticatingXboxLive,
    FetchingXstsToken,
    FetchingMinecraftToken,
    GettingProfile,
}

#[derive(Debug, thiserror::Error)]
pub enum LoginFlowError {
    #[error("token Xsts issues")]
    XstsError(#[from] XstsTokenErrorType),
    #[error("unknown data store error")]
    UnknownError(String),
    #[error("you don't own the game")]
    GameNotOwned,
    #[error("having issues sending through the channel")]
    Senderror(#[from] tokio::sync::mpsc::error::SendError<LoginFlowEvent>),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

type OAuthClient = oauth2::Client<
    StandardErrorResponse<BasicErrorResponseType>,
    StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>,
    BasicTokenType,
    StandardTokenIntrospectionResponse<EmptyExtraTokenFields, BasicTokenType>,
    StandardRevocableToken,
    StandardErrorResponse<RevocationErrorResponseType>,
>;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct XboxLiveResponse {
    token: String,
    display_claims: XboxLiveDisplayClaims,
}

#[derive(Debug, Deserialize)]
struct XboxLiveDisplayClaims {
    xui: Vec<XboxLiveDisplayClaimsXui>,
}

#[derive(Debug, Deserialize)]
struct XboxLiveDisplayClaimsXui {
    uhs: String,
}

enum XstsResponse {
    Success(String),
    Error(XstsTokenError),
}

#[derive(Debug, Clone, Deserialize)]
struct XstsTokenError {
    #[serde(rename = "XErr")]
    xerr: XstsTokenErrorType,
}

/// Reference: [Unofficial Mojang Wiki](https://wiki.vg/Microsoft_Authentication_Scheme)
#[derive(Debug, Clone, Deserialize, thiserror::Error)]
#[repr(usize)]
pub enum XstsTokenErrorType {
    #[error("The account doesn't have an Xbox account. Once they sign up for one (or login through minecraft.net to create one) then they can proceed with the login. This shouldn't happen with accounts that have purchased Minecraft with a Microsoft account, as they would've already gone through that Xbox signup process.")]
    DoesNotHaveXboxAccount = 2_148_916_233,
    #[error("The account is from a country where Xbox Live is not available/banned.")]
    CountryNotAvailable = 2_148_916_235,
    #[error("The account needs adult verification on Xbox page. (South Korea)")]
    NeedsAdultVerificationKR1 = 2_148_916_236,
    #[error("The account needs adult verification on Xbox page. (South Korea)")]
    NeedsAdultVerificationKR2 = 2_148_916_237,
    #[error("The account is a child (under 18) and cannot proceed unless the account is added to a Family by an adult. This only seems to occur when using a custom Microsoft Azure application. When using the Minecraft launchers client id, this doesn't trigger.")]
    ChildAccount = 2_148_916_238,
}

#[derive(Debug, Deserialize)]
struct MinecraftTokenResponse {
    access_token: String,
    /// The number of seconds until the access token expires.
    expires_in: i64,
}

#[derive(Debug, Deserialize)]
pub struct MinecraftUserProfile {
    pub id: Uuid,
    pub name: String,
    pub skins: Vec<MinecraftSkin>,
    pub capes: Vec<MinecraftCape>,
}

pub async fn login_flow(
    skin: UnboundedSender<LoginFlowEvent>,
) -> Result<MinecraftAccount, LoginFlowError> {
    skin.send(LoginFlowEvent::Stage(LoginFlowStage::FetchingDeviceCode))?;

    // Device Code Flow
    let client = create_oauth_client()?;
    let device_auth_response = fetch_device_code(&client).await?;
    skin.send(LoginFlowEvent::DeviceCode(LoginFlowDeviceCode {
        verification_uri: device_auth_response.verification_uri().to_string(),
        user_code: device_auth_response.user_code().secret().to_string(),
    }))?;
    skin.send(LoginFlowEvent::Stage(LoginFlowStage::WaitingForUser))?;

    // Microsoft Authentication Flow
    let (microsoft_token, refresh_token) =
        fetch_microsoft_token(&client, &device_auth_response).await?;
    skin.send(LoginFlowEvent::Stage(
        LoginFlowStage::AuthenticatingXboxLive,
    ))?;

    // Xbox Live Authentication Flow
    let (xbl_token, user_hash) = authenticate_xbox_live(&microsoft_token).await?;
    skin.send(LoginFlowEvent::Stage(LoginFlowStage::FetchingXstsToken))?;

    // XSTS Authentication Flow
    let xsts_response = fetch_xsts_token(&xbl_token).await?;
    let xsts_token: String = match xsts_response {
        XstsResponse::Success(token) => {
            skin.send(LoginFlowEvent::Stage(
                LoginFlowStage::FetchingMinecraftToken,
            ))?;

            token
        }
        XstsResponse::Error(e) => {
            return Err(e.xerr.into());
        }
    };

    // Minecraft Authentication Flow
    let (mc_access_token, expires_in) = fetch_minecraft_token(&xsts_token, &user_hash).await?;
    skin.send(LoginFlowEvent::Stage(LoginFlowStage::GettingProfile))?;

    let profile = match get_user_profile(&mc_access_token).await {
        Ok(profile) => profile,
        Err(e) => {
            return Err(LoginFlowError::GameNotOwned);
        }
    };

    let account = MinecraftAccount {
        username: profile.name,
        uuid: profile.id,
        access_token: AccountToken {
            token: mc_access_token,
            expires_at: Utc::now().add(Duration::seconds(expires_in)).timestamp(),
        },
        refresh_token: AccountToken {
            token: refresh_token,
            expires_at: Utc::now().add(Duration::days(90)).timestamp(),
        },
        capes: profile.capes,
        skins: profile.skins,
    };

    Ok(account)
}

fn create_oauth_client() -> anyhow::Result<OAuthClient> {
    let client = BasicClient::new(
        ClientId::new(MSA_CLIENT_ID.to_owned()),
        None,
        AuthUrl::new(MSA_URL.to_owned())?,
        Some(TokenUrl::new(MSA_TOKEN_URL.to_owned())?),
    )
    .set_device_authorization_url(DeviceAuthorizationUrl::new(MSA_DEVICE_CODE_URL.to_owned())?);

    Ok(client)
}

async fn fetch_device_code(
    client: &OAuthClient,
) -> anyhow::Result<StandardDeviceAuthorizationResponse> {
    let response: StandardDeviceAuthorizationResponse = client
        .exchange_device_code()?
        .add_scope(Scope::new(MSA_SCOPE.to_owned()))
        .request_async(async_http_client)
        .await?;

    Ok(response)
}

async fn fetch_microsoft_token(
    client: &OAuthClient,
    auth_response: &StandardDeviceAuthorizationResponse,
) -> anyhow::Result<(String, String)> {
    let response: StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType> = client
        .exchange_device_access_token(auth_response)
        .request_async(async_http_client, sleep, None)
        .await?;

    Ok((
        response.access_token().secret().clone(),
        response
            .refresh_token()
            .context("refresh token not found")?
            .secret()
            .clone(),
    ))
}

async fn authenticate_xbox_live(ms_access_token: &str) -> anyhow::Result<(String, String)> {
    let client = reqwest::Client::new();

    let payload = json!({
        "Properties": {
            "AuthMethod": "RPS",
            "SiteName": "user.auth.xboxlive.com",
            "RpsTicket": format!("d={}", ms_access_token)
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT",
    });

    let response = client
        .post(XBOX_LIVE_AUTH_URL)
        .json(&payload)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .send()
        .await?;
    let response = response.json::<XboxLiveResponse>().await?;

    let xbl_token = response.token;
    let user_hash = response
        .display_claims
        .xui
        .first()
        .context("Xbox Live user hash not found")?
        .uhs
        .clone();

    Ok((xbl_token, user_hash))
}

async fn fetch_xsts_token(xbl_token: &str) -> anyhow::Result<XstsResponse> {
    let client = reqwest::Client::new();

    let payload = json!({
        "Properties": {
            "SandboxId": "RETAIL",
            "UserTokens": [xbl_token],
        },
        "RelyingParty": "rp://api.minecraftservices.com/",
        "TokenType": "JWT",
    });

    let response = client
        .post(XBOX_LIVE_XSTS_URL)
        .json(&payload)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .send()
        .await?;

    if response.status() == 401 {
        let response = response.json::<XstsTokenError>().await?;
        return Ok(XstsResponse::Error(response));
    }

    let response = response.json::<XboxLiveResponse>().await?;
    let xsts_token = response.token;

    Ok(XstsResponse::Success(xsts_token))
}

async fn fetch_minecraft_token(xsts_token: &str, user_hash: &str) -> anyhow::Result<(String, i64)> {
    let client = reqwest::Client::new();

    let payload = json!({
        "identityToken": format!("XBL3.0 x={};{}", user_hash, xsts_token),
    });

    let response = client
        .post(MINECRAFT_AUTH_URL)
        .json(&payload)
        .send()
        .await?;
    let response = response.json::<MinecraftTokenResponse>().await?;

    Ok((response.access_token, response.expires_in))
}

async fn get_user_profile(mc_access_token: &str) -> anyhow::Result<MinecraftUserProfile> {
    let client = reqwest::Client::new();

    let response = client
        .get(MINECRAFT_USER_PROFILE_URL)
        .bearer_auth(mc_access_token)
        .send()
        .await?;
    let response = response.json::<MinecraftUserProfile>().await?;

    Ok(response)
}
