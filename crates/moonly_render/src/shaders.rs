use bevy::{
    asset::{AssetServer, load_embedded_asset},
    prelude::*,
};

use crate::queue::MoonlyGBufferShader;

pub(crate) fn init_gbuffer_shader(mut commands: Commands, asset_server: Res<AssetServer>) {
    let shader = load_embedded_asset!(asset_server.as_ref(), "gbuffer.wesl");
    commands.insert_resource(MoonlyGBufferShader(shader));
}
