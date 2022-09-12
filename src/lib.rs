use gdnative::prelude::*;

mod voip;

fn init(handle: InitHandle) {
    handle.add_class::<voip::GodotVoip>();
}

godot_init!(init);