use audiopus::coder::{Decoder, Encoder};
use audiopus::packet::Packet;
use audiopus::{Channels, MutSignals, SampleRate};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::Device;
use cpal::Host;
use cpal::SampleFormat;
use cpal::Stream;
use cpal::SupportedStreamConfig;
use gdnative::api::{AudioStreamPlayer, PacketPeerUDP};
use gdnative::prelude::*;

use std::convert::{TryFrom, TryInto};
use std::net::UdpSocket;
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
    size: Arc<Mutex<usize>>,
    decoder: Decoder,
    encoder: Encoder,
    remote_address: Arc<Mutex<String>>,
}

lazy_static!{
    static ref DECODER: Arc<Mutex<Decoder>> = Arc::new(Mutex::new(Decoder::new(SampleRate::Hz48000, Channels::Mono).unwrap()));
    static ref RECEIVED_VOICE_DATA: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    static ref LAST_PORT: Arc<Mutex<u16>> = Arc::new(Mutex::new(4200));
    static ref SOCKET: Arc<Mutex<UdpSocket>> = Arc::new(Mutex::new(UdpSocket::bind("0.0.0.0:8383").unwrap()));
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
            size: Arc::new(Mutex::new(0)),
            current_mic_data: Arc::new(Mutex::new(Vec::new())),
            decoder: Decoder::new(SampleRate::Hz48000, Channels::Mono).unwrap(),
            encoder: Encoder::new(
                SampleRate::Hz48000,
                Channels::Mono,
                audiopus::Application::Voip,
            )
            .unwrap(),
            remote_address: Arc::new(Mutex::new(String::from("127.0.0.1:4242"))),
        };

        instance
    }

    #[method]
    fn _ready(&self, #[base] owner: TRef<Node>) {
        godot_print!("hello, world.");
        let timer = Timer::new();
        timer
            .connect(
                "timeout",
                owner,
                "_on_timeout",
                VariantArray::new_shared(),
                0,
            )
            .unwrap();
        timer.set_autostart(true);
        owner.add_child(timer, false);

        // let one_second_timer = Timer::new();
        // one_second_timer.connect("timeout", owner, "_one_second_timeout", VariantArray::new_shared(), 0).unwrap();
        // one_second_timer.set_autostart(true);
        // owner.add_child(one_second_timer, false);
    }

    #[method]
    fn set_remote_address(&self, address: String) {
        // match UdpSocket::bind(address){
        //     Ok(socket) => {
        //         let mut udp_socket = self.udp_socket.lock().unwrap();
        //         *udp_socket = Some(socket);
        //         true
        //     },
        //     Err(_) => {
        //         false
        //     }
        // }
        let mut remote_address = self.remote_address.lock().unwrap();
        *remote_address = address;
    }

    #[method]
    fn _on_timeout(&mut self) {
        // let mut size = self.size.lock().unwrap();
        // godot_print!("Input size: {}", &size);
        // *size = 0;
        let size = self.size.lock().unwrap();
        godot_print!("Update: {}", size);
        // let udp = self.udp.lock().unwrap();
        // if udp.is_connected_to_host() && udp.get_available_packet_count() > 0 {
        //     let encoded = udp.get_packet().to_vec();
        //     // for byte in {
        //     //     godot_print!("{}", byte);
        //     // }

        //     let packed_encoded = Packet::try_from(&encoded).unwrap();

        //     // // let mut decoded: Vec<i16> = Vec::new();
        //     // // let mut decoded_array: [i16; 1024] = [0; 1024];
        //     let mut decoded_vec: Vec<f32> = vec![0.0; 4098];
        //     let mut_decoded = MutSignals::try_from(&mut decoded_vec).unwrap();
        //     let decoded_size = self.decoder.decode_float(Some(packed_encoded), mut_decoded, false);
        //     match decoded_size {
        //         Ok(size) => {
        //             // godot_print!("usize: {}", u);
        //             // godot_print!("Decoded! {}", size);
        //             let mut mic_data = self.current_mic_data.lock().unwrap();
        //             *mic_data = decoded_vec[0..size].to_vec();
        //             match &self.output_stream {
        //                 Some(stream) => {
        //                     stream.play().unwrap();
        //                 },
        //                 None => {}
        //             }
        //         },
        //         Err(e) => {
        //             godot_print!("error: {}", e);
        //         }
        //     }
        // }

        // let mut mic_data = self.current_mic_data.lock().unwrap();
        // // let mic_len = &mic_data.len();
        // // godot_print!("Mic lenght: {}", mic_len);
        // let mut accumulator = Vec::new();
        // for mic in mic_data.iter(){
        //     let mut encoded_buffer: [u8; 1024] = [0; 1024];
        //     let encoded_len = self.encoder.encode_float(&mic, &mut encoded_buffer).unwrap_or(1);
        //     accumulator.extend(encoded_buffer[0..encoded_len].to_vec().iter());
        // }

        // let _packed_encoded = Packet::try_from(&accumulator).unwrap();

        // let mut decoded_vec = vec![0; accumulator.len()];
        // let mut_decoded = MutSignals::try_from(&mut decoded_vec).unwrap();
        // let decoded_size = self.decoder.decode(Some(packed_encoded), mut_decoded, false);

        // match decoded_size {
        //     Ok(size) => {
        //         godot_print!("Decoded size: {}", size);
        //     },
        //     Err(err) => {
        //         godot_print!("Decode error: {}", err);
        //     }
        // }

        // *mic_data = Vec::new();
        match &self.output_stream {
            Some(stream) => {
                stream.play().unwrap();
            }
            None => {}
        }
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
        // unsafe {self.emplace().base().call("cu", &[]);}
        // let teste = self.emplace().base();

        // let node = unsafe {
        //     node.node_ref.assume_safe()
        // };
        // let a = self.emplace();
        godot_print!("creating stuff");
        // self.owner = Some(Arc::new(Mutex::new(owner)));
        // let owner_arc = Arc::clone(&self.owner.as_ref().unwrap());
        // let o = Arc::new(Mutex::new(owner));
        // let owner_arc = Arc::clone(&Arc::new(Mutex::new(owner)));

        let encoder = Encoder::new(
            SampleRate::Hz48000,
            audiopus::Channels::Mono,
            audiopus::Application::Voip,
        )
        .unwrap();
        let encoder_arc = Arc::new(Mutex::new(encoder));

        // let decoder = Decoder::new(SampleRate::Hz48000, audiopus::Channels::Stereo).unwrap();
        let decoder_arc = Arc::new(Mutex::new(
            Decoder::new(SampleRate::Hz48000, Channels::Mono).unwrap(),
        ));

        // let player_arc = Arc::new(Mutex::new(player));
        // unsafe{owner.assume_safe().call("a", &[]);}

        // let self_arc = Arc::new(Mutex::new(&self.current_mic_data));
        let input_mic_data_arc = Arc::clone(&self.current_mic_data);

        let remote_address_arc = Arc::clone(&self.remote_address);

        // let send_socket = Arc::new(Mutex::new(UdpSocket::bind("0.0.0.0:8383").unwrap()));
        let last_sent_packet_id: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));

        let mic_data_arc = Arc::clone(&self.current_mic_data);
        let decoder = Arc::new(Mutex::new(
            Decoder::new(SampleRate::Hz48000, Channels::Mono).unwrap(),
        ));
        let size_arc = Arc::clone(&self.size);

        let input_stream = self
            .device
            .build_input_stream(
                &self.config.config(),
                move |data: &[f32], _| {
                    // input_mic_data_arc.lock().unwrap().push(data.to_vec());
                    // let mut mic_data = input_mic_data_arc.lock().unwrap();
                    // *mic_data = data.to_vec();
                    // for d in data {
                    //     godot_print!("{}", d)
                    // }
                    //TODO: async
                    // unsafe {node.call("cu", &[]);}
                    godot_print!("got data");
                    // self_arc.lock().unwrap().extend(data.iter());
                    let mut encoded = [0; 1024];
                    let e_size = encoder_arc.lock().unwrap().encode_float(data, &mut encoded);
                    match e_size {
                        Ok(size) => {
                            let mut last_id = last_sent_packet_id.lock().unwrap();
                            let mut message = last_id.clone().to_le_bytes().to_vec();
                            message.extend_from_slice(&encoded[0..size - 1]);

                            *last_id = last_id.clone() + 1;
                            let send_socket = Arc::clone(&SOCKET);

                            godot_print!("sending data");
                                match send_socket.lock().unwrap()
                                    .send_to(&message, remote_address_arc.lock().unwrap().as_str())
                                {
                                    Ok(_) => {
                                        let mut buffer: [u8; 256] = [0; 256];
                                        godot_print!("receiving...");
                                        let recv_socket = Arc::clone(&SOCKET);
                                        match recv_socket.lock().unwrap().recv_from(&mut buffer) {
                                            Ok((amt, _src)) => {
                                                godot_print!("received!");
                                                // godot_print!("received size: {}", amt);
                                                let recv_buf = buffer[..amt].to_vec();

                                                let packet_id = u32::from_le_bytes(
                                                    recv_buf[0..4].try_into().unwrap(),
                                                );
                                                godot_print!("Packet id: {}", packet_id);

                                                // *size_arc.lock().unwrap() =
                                                //     packet_id.try_into().unwrap();

                                                let encoded_buffer = recv_buf[4..].to_vec();

                                                let packed_encoded =
                                                    Packet::try_from(&encoded_buffer).unwrap();

                                                let mut decoded_vec: Vec<f32> = vec![0.0; 4098];
                                                let mut_decoded =
                                                    MutSignals::try_from(&mut decoded_vec).unwrap();
                                                let decoded_size =
                                                    DECODER.lock().unwrap().decode_float(
                                                        Some(packed_encoded),
                                                        mut_decoded,
                                                        false,
                                                    );
                                                match decoded_size {
                                                    Ok(size) => {
                                                        // godot_print!("usize: {}", u);
                                                        // godot_print!("Decoded! {}", size);
                                                        let mut mic_data =
                                                            RECEIVED_VOICE_DATA.lock().unwrap();
                                                        *mic_data =
                                                            decoded_vec[0..size - 1].to_vec();
                                                        // match &self.output_stream {
                                                        //     Some(stream) => {
                                                        //         stream.play().unwrap();
                                                        //     },
                                                        //     None => {}
                                                        // }
                                                    }
                                                    Err(e) => {
                                                        godot_print!("error: {}", e);
                                                    }
                                                }
                                            }
                                            Err(err) => {
                                                godot_print!("received error: {}", err);
                                            }
                                        };
                                    }
                                    Err(err) => {
                                        godot_print!("Socket error: {}", err);
                                    }
                                };
                            //remote_address_arc.lock().unwrap().as_str()

                            // let udp = udp_arc.lock().unwrap();
                            // if udp.is_connected_to_host() {
                            //     // godot_print!("Pre encoding size: {}", size);
                            //     match udp.put_packet(PoolArray::from_slice(&encoded[0..size])){
                            //         Ok(_) => {
                            //             // godot_print!("Sending data");
                            //         },
                            //         Err(err) => {
                            //             godot_print!("Sending packet error: {}", err);
                            //         }
                            //     }
                            // }
                        },
                        Err(err) => {
                            godot_print!("Pre encoding error: {}", err);
                        }
                    }

                    // godot_print!("result: {}", decoded.to_vec()[0]);
                    // match decoded_result{
                    //     Ok(_) => {},
                    //     Err(e) => {
                    //         godot_print!("decoding error: {}", e);
                    //     }
                    // }
                    // unsafe{owner_arc.lock().unwrap().assume_safe().emit_signal("microphone_data", &[decoded_vec.to_variant()]);}
                    // let decoded_result = decoder_arc.lock().unwrap().decode(Some((&encoded[..]).try_into().unwrap()), (&mut decoded[..]).try_into().unwrap(), false);
                },
                move |_err| {
                    // react to errors here.
                    godot_print!("error");
                },
            )
            .unwrap();

        input_stream.play().unwrap();
        // // self.emplace().base().emit_signal("microphone_data", &[data.owned_to_variant()]);
        self.input_stream = Some(input_stream);

        let mut output_device_names = Vec::new();

        for d in self.host.output_devices().unwrap() {
            output_device_names.push(d.name().unwrap());
        }
        godot_print!("output devices: {}", output_device_names.join(" "));

        let output_mic_data_arc = Arc::clone(&self.current_mic_data);
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
                    // godot_print!("i'm playing");
                    let mut i = 0;
                    let mic_data = output_mic_data_arc.lock().unwrap();
                    for sample in data.iter_mut() {
                        if i < mic_data.len() {
                            *sample = mic_data[i];
                            i += 1;
                        }
                    }
                },
                move |err| {
                    godot_print!("output error: {}", err);
                },
            )
            .unwrap();
        output_stream.play().unwrap();

        self.output_stream = Some(output_stream);

        // let udp = Arc::clone(&self.udp_socket);
        // thread::spawn(move || {
        //     let recv_udp = UdpSocket::bind("0.0.0.0:3838").unwrap();
        //     loop {
        //         let mut buffer: [u8; 256] = [0; 256];
        //         godot_print!("receiving...");
        //         match recv_udp.recv_from(&mut buffer) {
        //             Ok((amt, _src)) => {
        //                 // godot_print!("received size: {}", amt);
        //                 let recv_buf = buffer[..amt].to_vec();

        //                 let packet_id = u32::from_le_bytes(recv_buf[0..4].try_into().unwrap());
        //                 godot_print!("Packet id: {}", packet_id);

        //                 *size_arc.lock().unwrap() = packet_id.try_into().unwrap();

        //                 let encoded_buffer = recv_buf[4..].to_vec();

        //                 let packed_encoded = Packet::try_from(&encoded_buffer).unwrap();

        //                 let mut decoded_vec: Vec<f32> = vec![0.0; 4098];
        //                 let mut_decoded = MutSignals::try_from(&mut decoded_vec).unwrap();
        //                 let decoded_size = decoder.lock().unwrap().decode_float(
        //                     Some(packed_encoded),
        //                     mut_decoded,
        //                     false,
        //                 );
        //                 match decoded_size {
        //                     Ok(size) => {
        //                         // godot_print!("usize: {}", u);
        //                         // godot_print!("Decoded! {}", size);
        //                         let mut mic_data = mic_data_arc.lock().unwrap();
        //                         *mic_data = decoded_vec[0..size - 1].to_vec();
        //                         // match &self.output_stream {
        //                         //     Some(stream) => {
        //                         //         stream.play().unwrap();
        //                         //     },
        //                         //     None => {}
        //                         // }
        //                     }
        //                     Err(e) => {
        //                         godot_print!("error: {}", e);
        //                     }
        //                 }
        //             }
        //             Err(err) => {
        //                 godot_print!("received error: {}", err);
        //             }
        //         }

        //         // let recv_buf = &mut buffer[..amt];
        //         // mic_data_arc.lock().unwrap()
        // }
        // });

        godot_print!("stream set");
    }
}
