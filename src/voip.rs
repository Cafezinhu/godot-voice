use std::collections::HashMap;

use audiopus::MutSignals;
use audiopus::coder::{Encoder, Decoder};
use audiopus::packet::Packet;

use gdnative::api::networked_multiplayer_peer::ConnectionStatus;
use gdnative::api::{AudioServer, AudioEffectCapture, AudioStreamGeneratorPlayback};
use gdnative::prelude::*;

#[derive(NativeClass)]
#[inherit(Node)]
pub struct GodotVoip{
    microphone_effect: Option<Ref<AudioEffectCapture>>,
    audio_stream_playbacks: HashMap<i64, Ref<AudioStreamGeneratorPlayback>>,
    encoder: Encoder,
    decoder: Decoder,
    muted: bool,
    last_voice_id: u32
}

#[methods]
impl GodotVoip {
    fn new(_: &Node) -> Self {
        GodotVoip {
            microphone_effect: None,
            audio_stream_playbacks: HashMap::new(),
            encoder: Encoder::new(audiopus::SampleRate::Hz16000, audiopus::Channels::Mono, audiopus::Application::Voip).unwrap(),
            decoder: Decoder::new(audiopus::SampleRate::Hz16000, audiopus::Channels::Mono).unwrap(),
            muted: false,
            last_voice_id: 0
        }
    }

    #[method]
    fn _process(&mut self, #[base] base: &Node, _delta: f64){
        if self.muted {
            return;
        }
        let tree = unsafe {base.get_tree().unwrap().assume_safe()};
        let peer_id;
        match tree.network_peer(){
            Some(network_peer) => {
                let safe_peer = unsafe {network_peer.assume_safe()};
                if safe_peer.get_connection_status() != ConnectionStatus::CONNECTED {
                    return;
                }
                peer_id = tree.get_network_unique_id();
            },
            None => {
                return;
            }
        }

        match &self.microphone_effect {
            Some(microphone_effect) => {
                let safe_effect = unsafe{ microphone_effect.assume_safe() };
                if safe_effect.get_frames_available() >= 960 {
                    let stereo_buffer = safe_effect.get_buffer(960);
                    let mono_buffer: Vec<f32> = stereo_buffer.to_vec().iter().map(|value| value.x).collect();

                    let buffer = mono_buffer.as_slice();
                    let mut encoded_buffer = [0u8; 960];
                    match self.encoder.encode_float(buffer, &mut encoded_buffer){
                        Ok(size) => {
                            let encoded_buffer = encoded_buffer[..size].to_vec();
                            let pool_variant = PoolArray::from_vec(encoded_buffer).to_variant();
                            let id = self.last_voice_id;
                            base.rpc_unreliable(GodotString::from_str("receive_voice"), &[peer_id.to_variant(), id.to_variant(), pool_variant]);
                            self.last_voice_id += 1;
                        },
                        Err(err) => {
                            godot_print!("Encoding error: {}", err);
                        }
                    }
                }
            },
            None => {}
        }
    }

    #[method]
    fn set_muted(&mut self, muted: bool){
        self.muted = muted;
    }

    #[method]
    fn get_muted(&self) -> bool {
        self.muted
    }

    #[method]
    fn set_bus_index(&mut self, index: i64) -> bool {
        let bus_effect = AudioServer::get_bus_effect(AudioServer::godot_singleton(), index, 0);
        match bus_effect {
            Some(effect) => {
                self.microphone_effect = Some(effect.cast::<AudioEffectCapture>().unwrap());
                return true;
            },
            None => {
                return false;
            }
        }
    }

    #[method]
    fn set_peer_audio_stream_playback(&mut self, peer_id: i64, audio_stream_playback: Ref<AudioStreamGeneratorPlayback>){
        self.audio_stream_playbacks.insert(peer_id, audio_stream_playback);
        let array = VariantArray::new();
        array.push(peer_id);
    }

    #[method]
    fn remove_peer_audio_stream_playback(&mut self, peer_id: i64) -> bool{
        match self.audio_stream_playbacks.remove(&peer_id) {
            Some(_) => {
                return true;
            },
            None => {
                return false;
            }
        }
    }

    #[method(rpc = "remote")]
    fn receive_voice(&mut self, peer_id: i64, encoded_buffer: PoolArray<u8>){
        let encoded_vec = encoded_buffer.to_vec();
        let packet_encoded = Packet::try_from(&encoded_vec).unwrap();

        let mut decoded_buffer: Vec<f32> = vec![0.0; 1024];
        let signal_buffer = MutSignals::try_from(&mut decoded_buffer).unwrap();

        match self.decoder.decode_float(Some(packet_encoded), signal_buffer, false){
            Ok(size) => {
                let buffer = &decoded_buffer[..size];
                let vector2_buffer: Vec<Vector2> = buffer.into_iter().map(|value| Vector2{x: value.clone(), y: value.clone()}).collect();
                let pool = PoolArray::from_vec(vector2_buffer);

                match self.audio_stream_playbacks.get(&peer_id) {
                    Some(audio_stream_playback) => {
                        let safe_playback = unsafe {audio_stream_playback.assume_safe()};
                        safe_playback.push_buffer(pool);
                    },
                    None => {}
                }
            },
            Err(err) => {
                godot_print!("Decoding error: {}", err);
            }
        }
    }
}
