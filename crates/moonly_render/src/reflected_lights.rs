use moonly_core::{MoonlyLight, MoonlyLightFlags, MoonlyLightType, VirtualGiLight};

pub(crate) fn local_light(light: &VirtualGiLight) -> MoonlyLight {
    MoonlyLight {
        light_type: MoonlyLightType::Point,
        flags: MoonlyLightFlags::VIRTUAL_GI,
        color: light.color,
        position: light.position,
        direction: light.direction,
        range: light.range,
        render_layers: light.render_layers,
        ..MoonlyLight::default()
    }
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    use super::*;

    #[test]
    fn reflected_lights_preserve_energy_range_and_direction() {
        let light = VirtualGiLight {
            position: Vec3::X,
            direction: Vec3::Y,
            color: Vec3::new(0.2, 0.3, 0.4),
            range: 2.5,
            render_layers: [2, 0, 0, 0],
        };
        let local = local_light(&light);
        assert_eq!(local.light_type, MoonlyLightType::Point);
        assert!(local.flags.contains(MoonlyLightFlags::VIRTUAL_GI));
        assert_eq!(local.color, light.color);
        assert!((local.range - light.range).abs() < f32::EPSILON);
        assert_eq!(local.direction, light.direction);
        assert_eq!(local.render_layers, light.render_layers);
    }
}
