use std::cell::RefCell;
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
#[register_with(Self::register_signals)]
pub struct GodotVoice{
    microphone_effect: Option<Ref<AudioEffectCapture>>,
    peer_configs: RefCell<HashMap<i64, PeerConfig>>,
    voice_packets: RefCell<HashMap<i64, Vec<VoicePacket>>>,
    sorted_voice_packets: RefCell<HashMap<i64, Vec<VoicePacket>>>,
    encoder: Encoder,
    decoder: RefCell<Decoder>,
    resampler: RefCell<SincFixedIn<f32>>,
    muted: bool,
    last_voice_id: RefCell<u32>,
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
impl GodotVoice {
    fn new(_: &Node) -> Self {
        GodotVoice {
            microphone_effect: None,
            peer_configs: RefCell::new(HashMap::new()),
            voice_packets: RefCell::new(HashMap::new()),
            sorted_voice_packets: RefCell::new(HashMap::new()),
            encoder: Encoder::new(audiopus::SampleRate::Hz16000, audiopus::Channels::Mono, audiopus::Application::Voip).unwrap(),
            decoder: RefCell::new(Decoder::new(audiopus::SampleRate::Hz16000, audiopus::Channels::Mono).unwrap()),
            resampler: RefCell::new(SincFixedIn::<f32>::new(
                16000 as f64 / 44100 as f64,
                3.0,
                INTERPOLATIONPARAMS,
                2646,
                1,
            ).unwrap()),
            muted: false,
            last_voice_id: RefCell::new(0),
            server_mode: false,
            jitter_buffer_delay_sec: 0.42
        }
    }

    fn register_signals(builder: &ClassBuilder<Self>) {
        builder
            .signal("voice_received")
            .with_param("peer_id", VariantType::I64)
            .with_param("voice_packet_id", VariantType::I64)
            .with_param("voice_buffer", VariantType::ByteArray)
            .done();
    }

    #[method]
    fn _ready(&self, #[base] base: TRef<Node>){
        if self.server_mode {
            return;
        }
        unsafe{base.get_tree().unwrap().assume_safe().create_timer(self.jitter_buffer_delay_sec, false).unwrap().assume_safe().connect("timeout", base, "loop_sort_voice_packets", VariantArray::new_shared(), 0).unwrap()};
    }

    #[method]
    fn _process(&self, #[base] base: &Node, _delta: f64){
        if self.server_mode {
            return;
        }
        let mut sorted_voice_packets = self.sorted_voice_packets.borrow_mut();
        for (k, mut v) in sorted_voice_packets.clone(){
            if let Some(peer_config) = self.peer_configs.borrow_mut().get(&k){
                if v.len() >= 1 {
                    let safe_playback = unsafe {peer_config.stream_playback.assume_safe()};
                    if safe_playback.can_push_buffer(960){
                        safe_playback.push_buffer(v[0].voice_pool.clone());
                        v.remove(0);
                        sorted_voice_packets.insert(k, v);
                    }
                }
            }
        }
        
        if self.muted {
            return;
        }

        let tree = unsafe {base.get_tree().unwrap().assume_safe()};
        if let Some(network_peer) = tree.network_peer(){
            let safe_peer = unsafe {network_peer.assume_safe()};
            if safe_peer.get_connection_status() != ConnectionStatus::CONNECTED {
                return;
            }
        }else{
            return;
        }

        if let Some(microphone_effect) = &self.microphone_effect{
            let safe_effect = unsafe{ microphone_effect.assume_safe() };
            if safe_effect.get_frames_available() < 2646 {
                return;
            }

            let stereo_buffer = safe_effect.get_buffer(2646);
            let mono_buffer: Vec<Vec<f32>> = vec![stereo_buffer.to_vec().iter().map(|value| value.x).collect()];

            let resampled_buffer = self.resampler.borrow_mut().process(&mono_buffer, None).unwrap();

            let buffer = resampled_buffer[0].as_slice();
            let mut encoded_buffer = [0u8; 960];
            if let Ok(size) = self.encoder.encode_float(buffer, &mut encoded_buffer){
                let encoded_buffer = encoded_buffer[..size].to_vec();
                let pool_variant = PoolArray::from_vec(encoded_buffer).to_variant();
                let mut id = self.last_voice_id.borrow_mut();
                base.rpc_unreliable("send_voice", &[id.to_variant(), pool_variant]);
                *id += 1;
            }
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
    fn set_peer_audio_stream_playback(&self, peer_id: i64, audio_stream_playback: Ref<AudioStreamGeneratorPlayback>){
        self.peer_configs.borrow_mut().insert(peer_id, PeerConfig {
            playback_enabled: true,
            stream_playback: audio_stream_playback
        });
        self.voice_packets.borrow_mut().insert(peer_id, Vec::new());
        self.sorted_voice_packets.borrow_mut().insert(peer_id, Vec::new());
    }

    #[method]
    fn set_peer_playback_enabled(&self, peer_id: i64, value: bool){
        let mut peer_configs = self.peer_configs.borrow_mut();
        if let Some(peer_config) = peer_configs.get(&peer_id){
            let mut new_config = peer_config.clone();
            new_config.playback_enabled = value;
            peer_configs.insert(peer_id, new_config);
        }
        else{
            godot_error!("Peer {} not found", peer_id);
        }
    }

    #[method]
    fn loop_sort_voice_packets(&self, #[base] base: TRef<Node>){
        let mut voice_packets = self.voice_packets.borrow_mut();
        for (k, v) in voice_packets.clone() {
            let mut sorted_voice_packets = v;
            sorted_voice_packets.sort_unstable_by_key(|value| value.id);
            self.sorted_voice_packets.borrow_mut().insert(k, sorted_voice_packets);
            voice_packets.insert(k, Vec::new());
        }

        unsafe{base.get_tree().unwrap().assume_safe().create_timer(self.jitter_buffer_delay_sec, false).unwrap().assume_safe().connect("timeout", base, "loop_sort_voice_packets", VariantArray::new_shared(), 0).unwrap()};
    }

    #[method]
    fn remove_peer_audio_stream_playback(&self, peer_id: i64) -> bool{
        match self.voice_packets.borrow_mut().remove(&peer_id){
            Some(_) => {},
            None => {}
        }
        match self.sorted_voice_packets.borrow_mut().remove(&peer_id){
            Some(_) => {},
            None => {}
        }
        match self.peer_configs.borrow_mut().remove(&peer_id) {
            Some(_) => {
                return true;
            },
            None => {
                return false;
            }
        }
    }

    #[method(rpc = "master")]
    fn send_voice(&self, #[base] base: TRef<Node>, voice_packet_id: u32, voice_buffer: PoolArray<u8>){
        let peer_id = unsafe{base.get_tree().unwrap().assume_safe().get_rpc_sender_id()};
        base.emit_signal("voice_received", &[peer_id.to_variant(), voice_packet_id.to_variant(), voice_buffer.to_variant()]);
    }

    #[method(rpc = "puppet_sync")]
    fn receive_voice(&self, peer_id: i64, voice_packet_id: u32, encoded_buffer: PoolArray<u8>){
        if self.server_mode {
            return;
        }

        if let Some(peer_config) = self.peer_configs.borrow_mut().get(&peer_id){
            if !peer_config.playback_enabled {
                return;
            }

            let encoded_vec = encoded_buffer.to_vec();
            let packet_encoded = Packet::try_from(&encoded_vec).unwrap();

            let mut decoded_buffer: Vec<f32> = vec![0.0; 1024];
            let signal_buffer = MutSignals::try_from(&mut decoded_buffer).unwrap();

            let decode_result = self.decoder.borrow_mut().decode_float(Some(packet_encoded), signal_buffer, false);
            if let Ok(size) = decode_result{
                let buffer = &decoded_buffer[..size];
                let vector2_buffer: Vec<Vector2> = buffer.into_iter().map(|value| Vector2{x: value.clone(), y: value.clone()}).collect();
                let pool = PoolArray::from_vec(vector2_buffer);
                let mut borrowed_voice_packets = self.voice_packets.borrow_mut();
                if let Some(voice_packets) = borrowed_voice_packets.get(&peer_id){
                    let mut new_voice_packets = voice_packets.to_vec();
                    new_voice_packets.push(VoicePacket { id: voice_packet_id, voice_pool: pool });
                    borrowed_voice_packets.insert(peer_id, new_voice_packets);
                }else{
                    godot_warn!("Voice packet from {} received. AudioStreamGeneratorPlayback not set with set_peer_audio_stream_playback.", peer_id);
                }
            }else if let Err(err) = decode_result{
                godot_warn!("Decoding error: {}", err);
            }
        }
    }
}
