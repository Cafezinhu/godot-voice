use std::collections::HashMap;

use audiopus::MutSignals;
use audiopus::coder::{Encoder, Decoder};
use audiopus::packet::Packet;

use rubato::{Resampler, SincFixedIn, InterpolationType, InterpolationParameters, WindowFunction};

use gdnative::api::networked_multiplayer_peer::ConnectionStatus;
use gdnative::api::{AudioServer, AudioEffectCapture, AudioStreamGeneratorPlayback};
use gdnative::prelude::*;

const INTERPOLATIONPARAMS: InterpolationParameters = InterpolationParameters {
    sinc_len: 256,
    f_cutoff: 0.95,
    interpolation: InterpolationType::Linear,
    oversampling_factor: 256,
    window: WindowFunction::BlackmanHarris2,
};

#[derive(NativeClass)]
#[inherit(Node)]
pub struct GodotVoip{
    microphone_effect: Option<Ref<AudioEffectCapture>>,
    peer_configs: HashMap<i64, PeerConfig>,
    voice_packets: HashMap<i64, Vec<VoicePacket>>,
    sorted_voice_packets: HashMap<i64, Vec<VoicePacket>>,
    encoder: Encoder,
    decoder: Decoder,
    resampler: SincFixedIn<f32>,
    muted: bool,
    last_voice_id: u32,
    server_mode: bool,
    jitter_buffer_delay_sec: f64
}

#[derive(Clone)]
struct PeerConfig{
    playback_enabled: bool,
    stream_playback: Ref<AudioStreamGeneratorPlayback>
}

#[derive(Clone)]
struct VoicePacket{
    id: u32,
    voice_pool: PoolArray<Vector2>
}

#[methods]
impl GodotVoip {
    fn new(_: &Node) -> Self {
        GodotVoip {
            microphone_effect: None,
            peer_configs: HashMap::new(),
            voice_packets: HashMap::new(),
            sorted_voice_packets: HashMap::new(),
            encoder: Encoder::new(audiopus::SampleRate::Hz16000, audiopus::Channels::Mono, audiopus::Application::Voip).unwrap(),
            decoder: Decoder::new(audiopus::SampleRate::Hz16000, audiopus::Channels::Mono).unwrap(),
            resampler: SincFixedIn::<f32>::new(
                16000 as f64 / 44100 as f64,
                3.0,
                INTERPOLATIONPARAMS,
                2646,
                1,
            ).unwrap(),
            muted: false,
            last_voice_id: 0,
            server_mode: false,
            jitter_buffer_delay_sec: 0.42
        }
    }

    #[method]
    fn _ready(&self, #[base] base: TRef<Node>){
        if self.server_mode {
            return;
        }
        unsafe{base.get_tree().unwrap().assume_safe().create_timer(self.jitter_buffer_delay_sec, false).unwrap().assume_safe().connect("timeout", base, "loop_sort_voice_packets", VariantArray::new_shared(), 0).unwrap()};
    }

    #[method]
    fn _process(&mut self, #[base] base: &Node, _delta: f64){
        if self.server_mode {
            return;
        }
        for (k, mut v) in self.sorted_voice_packets.clone(){
            match self.peer_configs.get(&k) {
                Some(peer_config) => {
                    if v.len() >= 1 {
                        let safe_playback = unsafe {peer_config.stream_playback.assume_safe()};
                        if safe_playback.can_push_buffer(960){
                            safe_playback.push_buffer(v[0].voice_pool.clone());
                            v.remove(0);
                            self.sorted_voice_packets.insert(k, v);
                        }
                    }
                },
                None => {}
            }
        }
        
        if self.muted {
            return;
        }
        let tree = unsafe {base.get_tree().unwrap().assume_safe()};
        match tree.network_peer(){
            Some(network_peer) => {
                let safe_peer = unsafe {network_peer.assume_safe()};
                if safe_peer.get_connection_status() != ConnectionStatus::CONNECTED {
                    return;
                }
            },
            None => {
                return;
            }
        }

        match &self.microphone_effect {
            Some(microphone_effect) => {
                let safe_effect = unsafe{ microphone_effect.assume_safe() };
                if safe_effect.get_frames_available() >= 2646 {
                    let stereo_buffer = safe_effect.get_buffer(2646);
                    let mono_buffer: Vec<Vec<f32>> = vec![stereo_buffer.to_vec().iter().map(|value| value.x).collect()];

                    let resampled_buffer = self.resampler.process(&mono_buffer, None).unwrap();

                    let buffer = resampled_buffer[0].as_slice();
                    let mut encoded_buffer = [0u8; 960];
                    match self.encoder.encode_float(buffer, &mut encoded_buffer){
                        Ok(size) => {
                            let encoded_buffer = encoded_buffer[..size].to_vec();
                            let pool_variant = PoolArray::from_vec(encoded_buffer).to_variant();
                            let id = self.last_voice_id;
                            base.rpc_unreliable("receive_voice", &[id.to_variant(), pool_variant]);
                            self.last_voice_id += 1;
                        },
                        Err(_) => {}
                    }
                }
            },
            None => {}
        }
    }

    #[method]
    fn set_jitter_buffer_delay_sec(&mut self, delay_sec: f64){
        self.jitter_buffer_delay_sec = delay_sec;
    }

    #[method]
    fn get_jitter_buffer_delay_sec(&self) -> f64{
        self.jitter_buffer_delay_sec
    }


    #[method]
    fn set_server_mode(&mut self, mode: bool){
        self.server_mode = mode;
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
        self.peer_configs.insert(peer_id, PeerConfig {
            playback_enabled: true,
            stream_playback: audio_stream_playback
        });
        self.voice_packets.insert(peer_id, Vec::new());
        self.sorted_voice_packets.insert(peer_id, Vec::new());
    }

    #[method]
    fn set_peer_playback_enabled(&mut self, peer_id: i64, value: bool){
        if let Some(peer_config) = self.peer_configs.get(&peer_id){
            let mut new_config = peer_config.clone();
            new_config.playback_enabled = value;
            self.peer_configs.insert(peer_id, new_config);
        }
        else{
            godot_error!("Peer {} not found", peer_id);
        }
    }

    #[method]
    fn loop_sort_voice_packets(&mut self, #[base] base: TRef<Node>){
        for (k, v) in self.voice_packets.clone() {
            let mut sorted_voice_packets = v;
            sorted_voice_packets.sort_unstable_by_key(|value| value.id);
            self.sorted_voice_packets.insert(k, sorted_voice_packets);
            self.voice_packets.insert(k, Vec::new());
        }

        unsafe{base.get_tree().unwrap().assume_safe().create_timer(self.jitter_buffer_delay_sec, false).unwrap().assume_safe().connect("timeout", base, "loop_sort_voice_packets", VariantArray::new_shared(), 0).unwrap()};
    }

    #[method]
    fn remove_peer_audio_stream_playback(&mut self, peer_id: i64) -> bool{
        match self.voice_packets.remove(&peer_id){
            Some(_) => {},
            None => {}
        }
        match self.sorted_voice_packets.remove(&peer_id){
            Some(_) => {},
            None => {}
        }
        match self.peer_configs.remove(&peer_id) {
            Some(_) => {
                return true;
            },
            None => {
                return false;
            }
        }
    }

    #[method(rpc = "remote")]
    fn receive_voice(&mut self, #[base] base: TRef<Node>, voice_packet_id: u32, encoded_buffer: PoolArray<u8>){
        if self.server_mode {
            return;
        }

        let peer_id = unsafe{base.get_tree().unwrap().assume_safe().get_rpc_sender_id()};

        match self.peer_configs.get(&peer_id) {
            Some(peer_config) => {

                if !peer_config.playback_enabled {
                    return;
                }

                let encoded_vec = encoded_buffer.to_vec();
                let packet_encoded = Packet::try_from(&encoded_vec).unwrap();

                let mut decoded_buffer: Vec<f32> = vec![0.0; 1024];
                let signal_buffer = MutSignals::try_from(&mut decoded_buffer).unwrap();

                match self.decoder.decode_float(Some(packet_encoded), signal_buffer, false){
                    Ok(size) => {
                        let buffer = &decoded_buffer[..size];
                        let vector2_buffer: Vec<Vector2> = buffer.into_iter().map(|value| Vector2{x: value.clone(), y: value.clone()}).collect();
                        let pool = PoolArray::from_vec(vector2_buffer);

                        match self.voice_packets.get(&peer_id){
                            Some(voice_packets) => {
                                let mut new_voice_packets = voice_packets.to_vec();
                                new_voice_packets.push(VoicePacket { id: voice_packet_id, voice_pool: pool });
                                self.voice_packets.insert(peer_id, new_voice_packets);
                            },
                            None => {
                                godot_warn!("Voice packet from {} received. AudioStreamGeneratorPlayback not set with set_peer_audio_stream_playback.", peer_id);
                            }
                        }
                    },
                    Err(err) => {
                        godot_print!("Decoding error: {}", err);
                    }
                }
            },
            None => {}
        }
    }
}
