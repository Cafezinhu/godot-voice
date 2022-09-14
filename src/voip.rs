use audiopus::coder::{Decoder, Encoder};
use audiopus::packet::Packet;
use audiopus::{Channels, MutSignals, SampleRate};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::Device;
use cpal::Host;
use cpal::SampleFormat;
use cpal::Stream;
use cpal::SupportedStreamConfig;
use gdnative::prelude::*;
use laminar::{Socket, Packet as UDPPacket, SocketEvent};

use std::convert::{TryFrom, TryInto};
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(NativeClass)]
#[inherit(Node)]
#[register_with(Self::register_signals)]
pub struct GodotVoip {
    host: Host,
    device: Device,
    config: SupportedStreamConfig,
    input_stream: Option<Stream>,
    output_stream: Option<Stream>,
    current_mic_data: Arc<Mutex<Vec<f32>>>,
    remote_address: Arc<Mutex<String>>,
}

#[methods]
impl GodotVoip {
    fn register_signals(builder: &ClassBuilder<Self>) {
        builder
            .signal("microphone_data")
            .with_param_default("data", Variant::new(0))
            .done();
    }

    fn new(_owner: &Node) -> Self {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .expect("no default input device found");
        let mut _supported_config_range = device
            .supported_input_configs()
            .expect("error while querying configs");
        let config = _supported_config_range
            .next()
            .unwrap()
            .with_max_sample_rate();

        let instance = GodotVoip {
            host,
            device,
            config,
            input_stream: None,
            output_stream: None,
            current_mic_data: Arc::new(Mutex::new(Vec::new())),
            remote_address: Arc::new(Mutex::new(String::from("127.0.0.1:4242"))),
        };

        instance
    }

    #[method]
    fn _ready(&self) {
        godot_print!("hello, world.");
    }

    #[method]
    fn set_remote_address(&self, address: String) {
        let mut remote_address = self.remote_address.lock().unwrap();
        *remote_address = address;
    }

    #[method]
    fn get_selected_device(&self) -> String {
        self.device.name().unwrap()
    }

    #[method]
    fn get_devices(&self) -> Vec<String> {
        let devices = self.host.input_devices().unwrap();
        let mut device_names = Vec::new();

        for device in devices.into_iter() {
            device_names.push(device.name().unwrap());
        }

        device_names
    }

    #[method]
    fn select_device(&mut self, name: String) {
        let devices = self.host.devices().unwrap();

        for device in devices.into_iter() {
            if device.name().unwrap() == name {
                self.device = device;

                let mut supported_configs_range = self
                    .device
                    .supported_input_configs()
                    .expect("error while querying configs");

                self.config = supported_configs_range
                    .next()
                    .expect("no supported config?!")
                    .with_max_sample_rate();

                godot_print!("Selected device: {}", name);
            }
        }
    }

    #[method]
    fn play(&self) {
        self.input_stream.as_ref().unwrap().play().unwrap();
    }

    #[method]
    fn get_sample_rate(&self) -> u32 {
        self.config.config().sample_rate.0
    }

    #[method]
    fn get_sample_format(&self) -> String {
        match self.config.sample_format() {
            SampleFormat::I16 => String::from("I16"),
            SampleFormat::U16 => String::from("U16"),
            SampleFormat::F32 => String::from("F32"),
        }
    }

    #[method]
    fn build_input_stream(&mut self) {
        match &self.input_stream {
            Some(value) => {
                value.pause().unwrap();
            }
            None => {}
        }
        godot_print!("creating stuff");

        let encoder = Encoder::new(
            SampleRate::Hz48000,
            audiopus::Channels::Mono,
            audiopus::Application::Voip,
        )
        .unwrap();
        let encoder_arc = Arc::new(Mutex::new(encoder));

        let remote_address_arc = Arc::clone(&self.remote_address);

        let last_sent_packet_id: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));

        let mut socket = Socket::bind("0.0.0.0:8383").unwrap();
        let packet_sender = Arc::new(Mutex::new(socket.get_packet_sender()));
        let event_receiver = socket.get_event_receiver();
        let _thread = thread::spawn(move || socket.start_polling());

        let input_stream = match self.config.sample_format() {
            SampleFormat::F32 => {
                self
                .device
                .build_input_stream(
                    &self.config.config(),
                    move |data: &[f32], _| {
                        let mut encoded = [0; 1024];
                        let e_size = encoder_arc.lock().unwrap().encode_float(data, &mut encoded);
                        match e_size {
                            Ok(size) => {
                                let mut last_id = last_sent_packet_id.lock().unwrap();
                                let mut message = last_id.clone().to_le_bytes().to_vec();
                                message.extend_from_slice(&encoded[0..size - 1]);

                                *last_id = last_id.clone() + 1;

                                let unreliable = UDPPacket::unreliable(remote_address_arc.lock().unwrap().as_str().parse().unwrap(), message);
                                match packet_sender.lock().unwrap().send(unreliable){
                                    Ok(_) => {
                                    },
                                    Err(err) => {
                                        godot_print!("Error sending packet: {}", err);
                                    }
                                }
                            },
                            Err(err) => {
                                godot_print!("Pre encoding error: {}", err);
                            }
                        }
                    },
                    move |_err| {
                        // react to errors here.
                        godot_print!("error");
                    },
                )
                .unwrap()
            },
            SampleFormat::I16 => {
                self
                .device
                .build_input_stream(
                    &self.config.config(),
                    move |data: &[i16], _| {
                        let mut encoded = [0; 1024];
                        let e_size = encoder_arc.lock().unwrap().encode(data, &mut encoded);
                        match e_size {
                            Ok(size) => {
                                let mut last_id = last_sent_packet_id.lock().unwrap();
                                let mut message = last_id.clone().to_le_bytes().to_vec();
                                message.extend_from_slice(&encoded[0..size - 1]);

                                *last_id = last_id.clone() + 1;

                                let unreliable = UDPPacket::unreliable(remote_address_arc.lock().unwrap().as_str().parse().unwrap(), message);
                                match packet_sender.lock().unwrap().send(unreliable){
                                    Ok(_) => {
                                    },
                                    Err(err) => {
                                        godot_print!("Error sending packet: {}", err);
                                    }
                                }
                            },
                            Err(err) => {
                                godot_print!("Pre encoding error: {}", err);
                            }
                        }
                    },
                    move |_err| {
                        // react to errors here.
                        godot_print!("error");
                    },
                )
                .unwrap()
            },
            SampleFormat::U16 => {
                self
                .device
                .build_input_stream(
                    &self.config.config(),
                    move |data: &[i16], _| {
                        let mut encoded = [0; 1024];
                        let e_size = encoder_arc.lock().unwrap().encode(data, &mut encoded);
                        match e_size {
                            Ok(size) => {
                                let mut last_id = last_sent_packet_id.lock().unwrap();
                                let mut message = last_id.clone().to_le_bytes().to_vec();
                                message.extend_from_slice(&encoded[0..size - 1]);

                                *last_id = last_id.clone() + 1;

                                let unreliable = UDPPacket::unreliable(remote_address_arc.lock().unwrap().as_str().parse().unwrap(), message);
                                match packet_sender.lock().unwrap().send(unreliable){
                                    Ok(_) => {
                                    },
                                    Err(err) => {
                                        godot_print!("Error sending packet: {}", err);
                                    }
                                }
                            },
                            Err(err) => {
                                godot_print!("Pre encoding error: {}", err);
                            }
                        }
                    },
                    move |_err| {
                        // react to errors here.
                        godot_print!("error");
                    },
                )
                .unwrap()
            }
        };
        
        input_stream.play().unwrap();
        self.input_stream = Some(input_stream);

        let output_mic_data_arc = Arc::clone(&self.current_mic_data);

        let decoder = Arc::new(Mutex::new(Decoder::new(SampleRate::Hz48000, Channels::Mono).unwrap()));

        thread::spawn(move || {
            loop {
                match event_receiver.recv(){
                    Ok(event) => {
                        match event {
                            SocketEvent::Packet(packet) => {
                                let msg = packet.payload();
                                let recv_buf = msg.to_vec();

                                let packet_id = u32::from_le_bytes(
                                    recv_buf[0..4].try_into().unwrap(),
                                );
                                godot_print!("Packet id: {}", packet_id);
                                let encoded_buffer = recv_buf[4..].to_vec();

                                let packed_encoded =
                                    Packet::try_from(&encoded_buffer).unwrap();

                                let mut decoded_vec: Vec<f32> = vec![0.0; 4098];
                                let mut_decoded =
                                    MutSignals::try_from(&mut decoded_vec).unwrap();
                                let decoded_size =
                                    decoder.lock().unwrap().decode_float(
                                        Some(packed_encoded),
                                        mut_decoded,
                                        false,
                                    );
                                match decoded_size {
                                    Ok(size) => {
                                        let mut mic_data =
                                            output_mic_data_arc.lock().unwrap();
                                        *mic_data =
                                            decoded_vec[0..size - 1].to_vec();
                                    }
                                    Err(e) => {
                                        godot_print!("error: {}", e);
                                    }
                                }
                            },
                            SocketEvent::Connect(_) => {
                                godot_print!("Connected!");
                            },
                            SocketEvent::Timeout(_) => {
                                godot_print!("Timeout!");
                            },
                            SocketEvent::Disconnect(_) => {
                                godot_print!("Disconnected!");
                            }
                        }
                        
                    },
                    Err(_) => {

                    }
                }
            }
        });

        let mic_data = Arc::clone(&self.current_mic_data);
        let output_device = self.host.default_output_device().unwrap();
        godot_print!("Selected output device: {}", output_device.name().unwrap());
        let mut output_supported_config = output_device.supported_output_configs().unwrap();
        let output_config = output_supported_config
            .next()
            .unwrap()
            .with_max_sample_rate();

        let output_stream = output_device
            .build_output_stream(
                &output_config.config(),
                move |data: &mut [f32], _| {
                    let mut i = 0;
                    let mut mic_data = mic_data.lock().unwrap();
                    for sample in data.iter_mut() {
                        if i < mic_data.len() {
                            *sample = mic_data[i];
                            i += 1;
                        }
                    }
                    *mic_data = Vec::new();
                },
                move |err| {
                    godot_print!("output error: {}", err);
                },
            )
            .unwrap();
        output_stream.play().unwrap();

        self.output_stream = Some(output_stream);

        godot_print!("stream set");
    }
}
