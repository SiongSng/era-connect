// This file is automatically generated, so please do not edit it.
// Generated by `flutter_rust_bridge`@ 2.0.0-dev.25.

// Section: imports

use super::*;
use crate::api::shared_resources::authentication::account::*;
use crate::api::shared_resources::collection::*;
use flutter_rust_bridge::for_generated::byteorder::{NativeEndian, ReadBytesExt, WriteBytesExt};
use flutter_rust_bridge::for_generated::transform_result_dco;
use flutter_rust_bridge::for_generated::wasm_bindgen;
use flutter_rust_bridge::for_generated::wasm_bindgen::prelude::*;
use flutter_rust_bridge::{Handler, IntoIntoDart};

// Section: boilerplate

flutter_rust_bridge::frb_generated_boilerplate_web!();

// Section: dart2rust

impl CstDecode<flutter_rust_bridge::for_generated::anyhow::Error> for String {
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> flutter_rust_bridge::for_generated::anyhow::Error {
        unimplemented!()
    }
}
impl CstDecode<String> for String {
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> String {
        self
    }
}
impl CstDecode<uuid::Uuid> for Box<[u8]> {
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> uuid::Uuid {
        let single: Vec<u8> = self.cst_decode();
        flutter_rust_bridge::for_generated::decode_uuid(single)
    }
}
impl CstDecode<crate::api::backend_exclusive::storage::account_storage::AccountStorageValue>
    for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue
{
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(
        self,
    ) -> crate::api::backend_exclusive::storage::account_storage::AccountStorageValue {
        let self_ = self.unchecked_into::<flutter_rust_bridge::for_generated::js_sys::Array>();
        match self_.get(0).unchecked_into_f64() as _ {
                    0 => { crate::api::backend_exclusive::storage::account_storage::AccountStorageValue::Accounts( self_.get(1).cst_decode()) },
1 => { crate::api::backend_exclusive::storage::account_storage::AccountStorageValue::MainAccount( self_.get(1).cst_decode()) },
                    _ => unreachable!(),
                }
    }
}
impl CstDecode<crate::api::shared_resources::authentication::account::AccountToken>
    for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue
{
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> crate::api::shared_resources::authentication::account::AccountToken {
        let self_ = self
            .dyn_into::<flutter_rust_bridge::for_generated::js_sys::Array>()
            .unwrap();
        assert_eq!(
            self_.length(),
            2,
            "Expected 2 elements, got {}",
            self_.length()
        );
        crate::api::shared_resources::authentication::account::AccountToken {
            token: self_.get(0).cst_decode(),
            expires_at: self_.get(1).cst_decode(),
        }
    }
}
impl CstDecode<crate::api::shared_resources::collection::AdvancedOptions>
    for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue
{
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> crate::api::shared_resources::collection::AdvancedOptions {
        let self_ = self
            .dyn_into::<flutter_rust_bridge::for_generated::js_sys::Array>()
            .unwrap();
        assert_eq!(
            self_.length(),
            1,
            "Expected 1 elements, got {}",
            self_.length()
        );
        crate::api::shared_resources::collection::AdvancedOptions {
            jvm_max_memory: self_.get(0).cst_decode(),
        }
    }
}
impl CstDecode<crate::api::shared_resources::collection::CollectionId>
    for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue
{
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> crate::api::shared_resources::collection::CollectionId {
        let self_ = self
            .dyn_into::<flutter_rust_bridge::for_generated::js_sys::Array>()
            .unwrap();
        assert_eq!(
            self_.length(),
            1,
            "Expected 1 elements, got {}",
            self_.length()
        );
        crate::api::shared_resources::collection::CollectionId(self_.get(0).cst_decode())
    }
}
impl CstDecode<crate::api::backend_exclusive::vanilla::launcher::LaunchArgs>
    for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue
{
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> crate::api::backend_exclusive::vanilla::launcher::LaunchArgs {
        let self_ = self
            .dyn_into::<flutter_rust_bridge::for_generated::js_sys::Array>()
            .unwrap();
        assert_eq!(
            self_.length(),
            3,
            "Expected 3 elements, got {}",
            self_.length()
        );
        crate::api::backend_exclusive::vanilla::launcher::LaunchArgs {
            jvm_args: self_.get(0).cst_decode(),
            main_class: self_.get(1).cst_decode(),
            game_args: self_.get(2).cst_decode(),
        }
    }
}
impl CstDecode<Vec<Collection>> for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue {
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> Vec<Collection> {
        self.dyn_into::<flutter_rust_bridge::for_generated::js_sys::Array>()
            .unwrap()
            .iter()
            .map(CstDecode::cst_decode)
            .collect()
    }
}
impl CstDecode<Vec<String>> for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue {
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> Vec<String> {
        self.dyn_into::<flutter_rust_bridge::for_generated::js_sys::Array>()
            .unwrap()
            .iter()
            .map(CstDecode::cst_decode)
            .collect()
    }
}
impl CstDecode<Vec<crate::api::shared_resources::authentication::account::MinecraftAccount>>
    for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue
{
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(
        self,
    ) -> Vec<crate::api::shared_resources::authentication::account::MinecraftAccount> {
        self.dyn_into::<flutter_rust_bridge::for_generated::js_sys::Array>()
            .unwrap()
            .iter()
            .map(CstDecode::cst_decode)
            .collect()
    }
}
impl CstDecode<Vec<crate::api::shared_resources::authentication::account::MinecraftCape>>
    for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue
{
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(
        self,
    ) -> Vec<crate::api::shared_resources::authentication::account::MinecraftCape> {
        self.dyn_into::<flutter_rust_bridge::for_generated::js_sys::Array>()
            .unwrap()
            .iter()
            .map(CstDecode::cst_decode)
            .collect()
    }
}
impl CstDecode<Vec<crate::api::shared_resources::authentication::account::MinecraftSkin>>
    for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue
{
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(
        self,
    ) -> Vec<crate::api::shared_resources::authentication::account::MinecraftSkin> {
        self.dyn_into::<flutter_rust_bridge::for_generated::js_sys::Array>()
            .unwrap()
            .iter()
            .map(CstDecode::cst_decode)
            .collect()
    }
}
impl CstDecode<Vec<u8>> for Box<[u8]> {
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> Vec<u8> {
        self.into_vec()
    }
}
impl CstDecode<Vec<crate::api::backend_exclusive::vanilla::version::VersionMetadata>>
    for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue
{
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> Vec<crate::api::backend_exclusive::vanilla::version::VersionMetadata> {
        self.dyn_into::<flutter_rust_bridge::for_generated::js_sys::Array>()
            .unwrap()
            .iter()
            .map(CstDecode::cst_decode)
            .collect()
    }
}
impl CstDecode<crate::api::shared_resources::authentication::msa_flow::LoginFlowDeviceCode>
    for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue
{
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(
        self,
    ) -> crate::api::shared_resources::authentication::msa_flow::LoginFlowDeviceCode {
        let self_ = self
            .dyn_into::<flutter_rust_bridge::for_generated::js_sys::Array>()
            .unwrap();
        assert_eq!(
            self_.length(),
            2,
            "Expected 2 elements, got {}",
            self_.length()
        );
        crate::api::shared_resources::authentication::msa_flow::LoginFlowDeviceCode {
            verification_uri: self_.get(0).cst_decode(),
            user_code: self_.get(1).cst_decode(),
        }
    }
}
impl CstDecode<crate::api::shared_resources::authentication::msa_flow::LoginFlowErrors>
    for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue
{
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> crate::api::shared_resources::authentication::msa_flow::LoginFlowErrors {
        let self_ = self.unchecked_into::<flutter_rust_bridge::for_generated::js_sys::Array>();
        match self_.get(0).unchecked_into_f64() as _ {
                    0 => { crate::api::shared_resources::authentication::msa_flow::LoginFlowErrors::XstsError( self_.get(1).cst_decode()) },
1 => crate::api::shared_resources::authentication::msa_flow::LoginFlowErrors::GameNotOwned,
2 => { crate::api::shared_resources::authentication::msa_flow::LoginFlowErrors::UnknownError( self_.get(1).cst_decode()) },
                    _ => unreachable!(),
                }
    }
}
impl CstDecode<crate::api::shared_resources::authentication::msa_flow::LoginFlowEvent>
    for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue
{
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> crate::api::shared_resources::authentication::msa_flow::LoginFlowEvent {
        let self_ = self.unchecked_into::<flutter_rust_bridge::for_generated::js_sys::Array>();
        match self_.get(0).unchecked_into_f64() as _ {
            0 => crate::api::shared_resources::authentication::msa_flow::LoginFlowEvent::Stage(
                self_.get(1).cst_decode(),
            ),
            1 => {
                crate::api::shared_resources::authentication::msa_flow::LoginFlowEvent::DeviceCode(
                    self_.get(1).cst_decode(),
                )
            }
            2 => crate::api::shared_resources::authentication::msa_flow::LoginFlowEvent::Error(
                self_.get(1).cst_decode(),
            ),
            3 => crate::api::shared_resources::authentication::msa_flow::LoginFlowEvent::Success(
                self_.get(1).cst_decode(),
            ),
            _ => unreachable!(),
        }
    }
}
impl CstDecode<crate::api::shared_resources::authentication::account::MinecraftAccount>
    for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue
{
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> crate::api::shared_resources::authentication::account::MinecraftAccount {
        let self_ = self
            .dyn_into::<flutter_rust_bridge::for_generated::js_sys::Array>()
            .unwrap();
        assert_eq!(
            self_.length(),
            6,
            "Expected 6 elements, got {}",
            self_.length()
        );
        crate::api::shared_resources::authentication::account::MinecraftAccount {
            username: self_.get(0).cst_decode(),
            uuid: self_.get(1).cst_decode(),
            access_token: self_.get(2).cst_decode(),
            refresh_token: self_.get(3).cst_decode(),
            skins: self_.get(4).cst_decode(),
            capes: self_.get(5).cst_decode(),
        }
    }
}
impl CstDecode<crate::api::shared_resources::authentication::account::MinecraftCape>
    for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue
{
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> crate::api::shared_resources::authentication::account::MinecraftCape {
        let self_ = self
            .dyn_into::<flutter_rust_bridge::for_generated::js_sys::Array>()
            .unwrap();
        assert_eq!(
            self_.length(),
            4,
            "Expected 4 elements, got {}",
            self_.length()
        );
        crate::api::shared_resources::authentication::account::MinecraftCape {
            id: self_.get(0).cst_decode(),
            state: self_.get(1).cst_decode(),
            url: self_.get(2).cst_decode(),
            alias: self_.get(3).cst_decode(),
        }
    }
}
impl CstDecode<crate::api::shared_resources::authentication::account::MinecraftSkin>
    for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue
{
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> crate::api::shared_resources::authentication::account::MinecraftSkin {
        let self_ = self
            .dyn_into::<flutter_rust_bridge::for_generated::js_sys::Array>()
            .unwrap();
        assert_eq!(
            self_.length(),
            4,
            "Expected 4 elements, got {}",
            self_.length()
        );
        crate::api::shared_resources::authentication::account::MinecraftSkin {
            id: self_.get(0).cst_decode(),
            state: self_.get(1).cst_decode(),
            url: self_.get(2).cst_decode(),
            variant: self_.get(3).cst_decode(),
        }
    }
}
impl CstDecode<crate::api::shared_resources::collection::ModLoader>
    for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue
{
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> crate::api::shared_resources::collection::ModLoader {
        let self_ = self
            .dyn_into::<flutter_rust_bridge::for_generated::js_sys::Array>()
            .unwrap();
        assert_eq!(
            self_.length(),
            2,
            "Expected 2 elements, got {}",
            self_.length()
        );
        crate::api::shared_resources::collection::ModLoader {
            mod_loader_type: self_.get(0).cst_decode(),
            version: self_.get(1).cst_decode(),
        }
    }
}
impl CstDecode<Option<uuid::Uuid>> for Option<Box<[u8]>> {
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> Option<uuid::Uuid> {
        self.map(CstDecode::cst_decode)
    }
}
impl CstDecode<crate::api::backend_exclusive::storage::global_settings::UILayoutValue>
    for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue
{
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> crate::api::backend_exclusive::storage::global_settings::UILayoutValue {
        let self_ = self.unchecked_into::<flutter_rust_bridge::for_generated::js_sys::Array>();
        match self_.get(0).unchecked_into_f64() as _ {
                    0 => { crate::api::backend_exclusive::storage::global_settings::UILayoutValue::CompletedSetup( self_.get(1).cst_decode()) },
1 => { crate::api::backend_exclusive::storage::global_settings::UILayoutValue::ShowsRecommendation( self_.get(1).cst_decode()) },
2 => { crate::api::backend_exclusive::storage::global_settings::UILayoutValue::SidebarPreivew( self_.get(1).cst_decode()) },
                    _ => unreachable!(),
                }
    }
}
impl CstDecode<crate::api::backend_exclusive::vanilla::version::VersionMetadata>
    for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue
{
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> crate::api::backend_exclusive::vanilla::version::VersionMetadata {
        let self_ = self
            .dyn_into::<flutter_rust_bridge::for_generated::js_sys::Array>()
            .unwrap();
        assert_eq!(
            self_.length(),
            7,
            "Expected 7 elements, got {}",
            self_.length()
        );
        crate::api::backend_exclusive::vanilla::version::VersionMetadata {
            id: self_.get(0).cst_decode(),
            version_type: self_.get(1).cst_decode(),
            url: self_.get(2).cst_decode(),
            uploaded_time: self_.get(3).cst_decode(),
            release_time: self_.get(4).cst_decode(),
            sha1: self_.get(5).cst_decode(),
            compliance_level: self_.get(6).cst_decode(),
        }
    }
}
impl CstDecode<flutter_rust_bridge::for_generated::anyhow::Error>
    for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue
{
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> flutter_rust_bridge::for_generated::anyhow::Error {
        unimplemented!()
    }
}
impl CstDecode<Collection> for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue {
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> Collection {
        CstDecode::<RustOpaqueNom<flutter_rust_bridge::for_generated::rust_async::RwLock<Collection>>>::cst_decode(self).rust_auto_opaque_decode_owned()
    }
}
impl CstDecode<PathBuf> for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue {
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> PathBuf {
        CstDecode::<RustOpaqueNom<flutter_rust_bridge::for_generated::rust_async::RwLock<PathBuf>>>::cst_decode(self).rust_auto_opaque_decode_owned()
    }
}
impl CstDecode<chrono::DateTime<chrono::Utc>>
    for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue
{
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> chrono::DateTime<chrono::Utc> {
        CstDecode::<i64>::cst_decode(self).cst_decode()
    }
}
impl CstDecode<RustOpaqueNom<flutter_rust_bridge::for_generated::rust_async::RwLock<Collection>>>
    for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue
{
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(
        self,
    ) -> RustOpaqueNom<flutter_rust_bridge::for_generated::rust_async::RwLock<Collection>> {
        #[cfg(target_pointer_width = "64")]
        {
            compile_error!("64-bit pointers are not supported.");
        }
        unsafe { decode_rust_opaque_nom((self.as_f64().unwrap() as usize) as _) }
    }
}
impl CstDecode<RustOpaqueNom<flutter_rust_bridge::for_generated::rust_async::RwLock<PathBuf>>>
    for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue
{
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(
        self,
    ) -> RustOpaqueNom<flutter_rust_bridge::for_generated::rust_async::RwLock<PathBuf>> {
        #[cfg(target_pointer_width = "64")]
        {
            compile_error!("64-bit pointers are not supported.");
        }
        unsafe { decode_rust_opaque_nom((self.as_f64().unwrap() as usize) as _) }
    }
}
impl CstDecode<String> for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue {
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> String {
        self.as_string().expect("non-UTF-8 string, or not a string")
    }
}
impl CstDecode<uuid::Uuid> for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue {
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> uuid::Uuid {
        self.unchecked_into::<flutter_rust_bridge::for_generated::js_sys::Uint8Array>()
            .to_vec()
            .into_boxed_slice()
            .cst_decode()
    }
}
impl CstDecode<crate::api::backend_exclusive::storage::account_storage::AccountStorageKey>
    for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue
{
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(
        self,
    ) -> crate::api::backend_exclusive::storage::account_storage::AccountStorageKey {
        (self.unchecked_into_f64() as i32).cst_decode()
    }
}
impl CstDecode<bool> for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue {
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> bool {
        self.is_truthy()
    }
}
impl CstDecode<i32> for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue {
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> i32 {
        self.unchecked_into_f64() as _
    }
}
impl CstDecode<i64> for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue {
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> i64 {
        ::std::convert::TryInto::try_into(
            self.dyn_into::<flutter_rust_bridge::for_generated::js_sys::BigInt>()
                .unwrap(),
        )
        .unwrap()
    }
}
impl CstDecode<Vec<u8>> for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue {
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> Vec<u8> {
        self.unchecked_into::<flutter_rust_bridge::for_generated::js_sys::Uint8Array>()
            .to_vec()
            .into()
    }
}
impl CstDecode<crate::api::shared_resources::authentication::msa_flow::LoginFlowStage>
    for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue
{
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> crate::api::shared_resources::authentication::msa_flow::LoginFlowStage {
        (self.unchecked_into_f64() as i32).cst_decode()
    }
}
impl CstDecode<crate::api::shared_resources::authentication::account::MinecraftSkinVariant>
    for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue
{
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(
        self,
    ) -> crate::api::shared_resources::authentication::account::MinecraftSkinVariant {
        (self.unchecked_into_f64() as i32).cst_decode()
    }
}
impl CstDecode<crate::api::shared_resources::collection::ModLoaderType>
    for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue
{
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> crate::api::shared_resources::collection::ModLoaderType {
        (self.unchecked_into_f64() as i32).cst_decode()
    }
}
impl CstDecode<u32> for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue {
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> u32 {
        self.unchecked_into_f64() as _
    }
}
impl CstDecode<u8> for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue {
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> u8 {
        self.unchecked_into_f64() as _
    }
}
impl CstDecode<crate::api::backend_exclusive::storage::global_settings::UILayoutKey>
    for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue
{
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> crate::api::backend_exclusive::storage::global_settings::UILayoutKey {
        (self.unchecked_into_f64() as i32).cst_decode()
    }
}
impl CstDecode<usize> for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue {
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> usize {
        self.unchecked_into_f64() as _
    }
}
impl CstDecode<crate::api::backend_exclusive::vanilla::version::VersionType>
    for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue
{
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(self) -> crate::api::backend_exclusive::vanilla::version::VersionType {
        (self.unchecked_into_f64() as i32).cst_decode()
    }
}
impl CstDecode<crate::api::shared_resources::authentication::msa_flow::XstsTokenErrorType>
    for flutter_rust_bridge::for_generated::wasm_bindgen::JsValue
{
    // Codec=Cst (C-struct based), see doc to use other codecs
    fn cst_decode(
        self,
    ) -> crate::api::shared_resources::authentication::msa_flow::XstsTokenErrorType {
        (self.unchecked_into_f64() as i32).cst_decode()
    }
}

#[wasm_bindgen]
pub fn wire_MinecraftSkin_get_head_file_path(
    port_: flutter_rust_bridge::for_generated::MessagePort,
    that: flutter_rust_bridge::for_generated::wasm_bindgen::JsValue,
) {
    wire_MinecraftSkin_get_head_file_path_impl(port_, that)
}

#[wasm_bindgen]
pub fn wire_Collection_create(
    port_: flutter_rust_bridge::for_generated::MessagePort,
    display_name: String,
    version_metadata: flutter_rust_bridge::for_generated::wasm_bindgen::JsValue,
    mod_loader: flutter_rust_bridge::for_generated::wasm_bindgen::JsValue,
    advanced_options: flutter_rust_bridge::for_generated::wasm_bindgen::JsValue,
) {
    wire_Collection_create_impl(
        port_,
        display_name,
        version_metadata,
        mod_loader,
        advanced_options,
    )
}

#[wasm_bindgen]
pub fn wire_Collection_download_game(
    port_: flutter_rust_bridge::for_generated::MessagePort,
    that: flutter_rust_bridge::for_generated::wasm_bindgen::JsValue,
) {
    wire_Collection_download_game_impl(port_, that)
}

#[wasm_bindgen]
pub fn wire_Collection_game_directory(
    port_: flutter_rust_bridge::for_generated::MessagePort,
    that: flutter_rust_bridge::for_generated::wasm_bindgen::JsValue,
) {
    wire_Collection_game_directory_impl(port_, that)
}

#[wasm_bindgen]
pub fn wire_Collection_get_base_path(port_: flutter_rust_bridge::for_generated::MessagePort) {
    wire_Collection_get_base_path_impl(port_)
}

#[wasm_bindgen]
pub fn wire_Collection_get_collection_id(
    port_: flutter_rust_bridge::for_generated::MessagePort,
    that: flutter_rust_bridge::for_generated::wasm_bindgen::JsValue,
) {
    wire_Collection_get_collection_id_impl(port_, that)
}

#[wasm_bindgen]
pub fn wire_Collection_launch_game(
    port_: flutter_rust_bridge::for_generated::MessagePort,
    that: flutter_rust_bridge::for_generated::wasm_bindgen::JsValue,
) {
    wire_Collection_launch_game_impl(port_, that)
}

#[wasm_bindgen]
pub fn wire_Collection_save(
    port_: flutter_rust_bridge::for_generated::MessagePort,
    that: flutter_rust_bridge::for_generated::wasm_bindgen::JsValue,
) {
    wire_Collection_save_impl(port_, that)
}

#[wasm_bindgen]
pub fn wire_Collection_scan(port_: flutter_rust_bridge::for_generated::MessagePort) {
    wire_Collection_scan_impl(port_)
}

#[wasm_bindgen]
pub fn wire_create_collection(
    port_: flutter_rust_bridge::for_generated::MessagePort,
    display_name: String,
    version_metadata: flutter_rust_bridge::for_generated::wasm_bindgen::JsValue,
    mod_loader: flutter_rust_bridge::for_generated::wasm_bindgen::JsValue,
    advanced_options: flutter_rust_bridge::for_generated::wasm_bindgen::JsValue,
) {
    wire_create_collection_impl(
        port_,
        display_name,
        version_metadata,
        mod_loader,
        advanced_options,
    )
}

#[wasm_bindgen]
pub fn wire_get_account_storage(
    key: i32,
) -> flutter_rust_bridge::for_generated::WireSyncRust2DartDco {
    wire_get_account_storage_impl(key)
}

#[wasm_bindgen]
pub fn wire_get_skin_file_path(
    skin: flutter_rust_bridge::for_generated::wasm_bindgen::JsValue,
) -> flutter_rust_bridge::for_generated::WireSyncRust2DartDco {
    wire_get_skin_file_path_impl(skin)
}

#[wasm_bindgen]
pub fn wire_get_ui_layout_storage(
    port_: flutter_rust_bridge::for_generated::MessagePort,
    key: i32,
) {
    wire_get_ui_layout_storage_impl(port_, key)
}

#[wasm_bindgen]
pub fn wire_get_vanilla_versions(port_: flutter_rust_bridge::for_generated::MessagePort) {
    wire_get_vanilla_versions_impl(port_)
}

#[wasm_bindgen]
pub fn wire_init_app(port_: flutter_rust_bridge::for_generated::MessagePort) {
    wire_init_app_impl(port_)
}

#[wasm_bindgen]
pub fn wire_minecraft_login_flow(port_: flutter_rust_bridge::for_generated::MessagePort) {
    wire_minecraft_login_flow_impl(port_)
}

#[wasm_bindgen]
pub fn wire_remove_minecraft_account(
    port_: flutter_rust_bridge::for_generated::MessagePort,
    uuid: Box<[u8]>,
) {
    wire_remove_minecraft_account_impl(port_, uuid)
}

#[wasm_bindgen]
pub fn wire_set_ui_layout_storage(
    port_: flutter_rust_bridge::for_generated::MessagePort,
    value: flutter_rust_bridge::for_generated::wasm_bindgen::JsValue,
) {
    wire_set_ui_layout_storage_impl(port_, value)
}

#[wasm_bindgen]
pub fn rust_arc_increment_strong_count_RustOpaque_flutter_rust_bridgefor_generatedrust_asyncRwLockCollection(
    ptr: *const std::ffi::c_void,
) {
    unsafe {
        StdArc::<flutter_rust_bridge::for_generated::rust_async::RwLock<Collection>>::increment_strong_count(ptr as _);
    }
}

#[wasm_bindgen]
pub fn rust_arc_decrement_strong_count_RustOpaque_flutter_rust_bridgefor_generatedrust_asyncRwLockCollection(
    ptr: *const std::ffi::c_void,
) {
    unsafe {
        StdArc::<flutter_rust_bridge::for_generated::rust_async::RwLock<Collection>>::decrement_strong_count(ptr as _);
    }
}

#[wasm_bindgen]
pub fn rust_arc_increment_strong_count_RustOpaque_flutter_rust_bridgefor_generatedrust_asyncRwLockPathBuf(
    ptr: *const std::ffi::c_void,
) {
    unsafe {
        StdArc::<flutter_rust_bridge::for_generated::rust_async::RwLock<PathBuf>>::increment_strong_count(ptr as _);
    }
}

#[wasm_bindgen]
pub fn rust_arc_decrement_strong_count_RustOpaque_flutter_rust_bridgefor_generatedrust_asyncRwLockPathBuf(
    ptr: *const std::ffi::c_void,
) {
    unsafe {
        StdArc::<flutter_rust_bridge::for_generated::rust_async::RwLock<PathBuf>>::decrement_strong_count(ptr as _);
    }
}
