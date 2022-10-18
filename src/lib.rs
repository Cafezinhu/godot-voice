use gdnative::prelude::*;

mod voip;

#[no_mangle]
pub unsafe extern "C" fn __cxa_pure_virtual() {
    loop {
         
    }
}

fn init(handle: InitHandle) {
    handle.add_class::<voip::GodotVoip>();
}

godot_init!(init);