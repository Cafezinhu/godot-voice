use gdnative::prelude::*;

mod voip;

fn init(handle: InitHandle) {
    handle.add_class::<voip::GodotVoice>();
}

godot_init!(init);
