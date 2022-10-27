use audiopus::coder::{Decoder, Encoder};
use audiopus::packet::Packet;
use audiopus::{Channels, MutSignals, SampleRate};
use gdnative::api::{AudioServer, AudioEffect, AudioEffectCapture};
use gdnative::prelude::*;
use laminar::{Socket, Packet as UDPPacket, SocketEvent};

use std::convert::{TryFrom, TryInto};
use std::sync::{Arc, Mutex};
use std::{thread, time};

#[derive(NativeClass)]
#[inherit(Node)]
#[register_with(Self::register_signals)]
pub struct GodotVoip {
    current_mic_data_f32: Arc<Mutex<Vec<f32>>>,
    current_mic_data_i16: Arc<Mutex<Vec<i16>>>,
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
        let instance = GodotVoip {
            current_mic_data_f32: Arc::new(Mutex::new(Vec::new())),
            current_mic_data_i16: Arc::new(Mutex::new(Vec::new())),
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

    // #[method]
    // fn get_selected_device(&self) -> String {
    //     self.device.name().unwrap()
    // }

    // #[method]
    // fn get_devices(&self) -> Vec<String> {
    //     let devices = self.host.input_devices().unwrap();
    //     let mut device_names = Vec::new();

    //     for device in devices.into_iter() {
    //         device_names.push(device.name().unwrap());
    //     }

    //     device_names
    // }

    // #[method]
    // fn select_device(&mut self, name: String) {
    //     let devices = self.host.devices().unwrap();

    //     for device in devices.into_iter() {
    //         if device.name().unwrap() == name {
    //             self.device = device;

    //             let mut supported_configs_range = self
    //                 .device
    //                 .supported_input_configs()
    //                 .expect("error while querying configs");

    //             self.config = supported_configs_range
    //                 .next()
    //                 .expect("no supported config?!")
    //                 .with_max_sample_rate();

    //             godot_print!("Selected device: {}", name);
    //         }
    //     }
    // }

    // #[method]
    // fn get_sample_rate(&self) -> u32 {
    //     self.config.config().sample_rate.0
    // }

    // #[method]
    // fn get_sample_format(&self) -> String {
    //     match self.config.sample_format() {
    //         SampleFormat::I16 => String::from("I16"),
    //         SampleFormat::U16 => String::from("U16"),
    //         SampleFormat::F32 => String::from("F32"),
    //     }
    // }

    #[method]
    #[cfg(target_arch = "x86_64")]
    fn build_input_stream(&mut self, id: i32, output_id: i32) {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        use cpal::SampleFormat;

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
        // match input_stream {
        //     Some(value) => {
        //         value.pause().unwrap();
        //     }
        //     None => {}
        // }
        godot_print!("creating stuff");

        let mut encoder = Encoder::new(
            SampleRate::Hz48000,
            audiopus::Channels::Stereo,
            audiopus::Application::Voip,
        )
        .unwrap();
        encoder.set_force_channels(audiopus::Channels::Stereo).unwrap();
        // encoder.set_max_bandwidth(audiopus::Bandwidth::Narrowband).unwrap();
        // encoder.set_force_channels(Channels::Mono);
        let encoder_arc = Arc::new(Mutex::new(encoder));

        let remote_address_arc = Arc::clone(&self.remote_address);

        let last_sent_packet_id: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));

        let mut socket = Socket::bind("0.0.0.0:0").unwrap();
        let packet_sender = Arc::new(Mutex::new(socket.get_packet_sender()));
        let event_receiver = socket.get_event_receiver();
        let _thread = thread::spawn(move || socket.start_polling());

        let input_stream = match config.sample_format() {
            SampleFormat::F32 => {
                device
                .build_input_stream(
                    &config.config(),
                    move |data: &[f32], _| {
                        let mut encoded = [0; 1024];
                        let e_size = encoder_arc.lock().unwrap().encode_float(data, &mut encoded);
                        match e_size {
                            Ok(size) => {
                                let mut last_id = last_sent_packet_id.lock().unwrap();
                                let mut message = last_id.clone().to_le_bytes().to_vec();
                                message.extend_from_slice(&encoded[0..size]);

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
                device
                .build_input_stream(
                    &config.config(),
                    move |data: &[i16], _| {
                        godot_print!("Getting data!: {}", data.len());
                        let mut encoded = [0; 10240];
                        
                        let e_size = encoder_arc.lock().unwrap().encode(data, &mut encoded);
                        match e_size {
                            Ok(size) => {
                                let mut last_id = last_sent_packet_id.lock().unwrap();
                                let mut message = last_id.clone().to_le_bytes().to_vec();
                                message.extend_from_slice(&encoded[0..size]);

                                godot_print!("Sending size: {}", message.len());

                                *last_id = last_id.clone() + 1;

                                let unreliable = UDPPacket::unreliable(remote_address_arc.lock().unwrap().as_str().parse().unwrap(), message);
                                match packet_sender.lock().unwrap().send(unreliable){
                                    Ok(_) => {
                                        godot_print!("Data sent!");
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
                device
                .build_input_stream(
                    &config.config(),
                    move |data: &[i16], _| {
                        let mut encoded = [0; 1024];
                        let e_size = encoder_arc.lock().unwrap().encode(data, &mut encoded);
                        match e_size {
                            Ok(size) => {
                                let mut last_id = last_sent_packet_id.lock().unwrap();
                                let mut message = last_id.clone().to_le_bytes().to_vec();
                                message.extend_from_slice(&encoded[0..size]);

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

        let mic_data_f32 = Arc::clone(&self.current_mic_data_f32);
        let mic_data_i16 = Arc::clone(&self.current_mic_data_i16);
        let output_device = host.default_output_device().unwrap();
        godot_print!("Selected output device: {}", output_device.name().unwrap());
        let mut output_supported_config = output_device.supported_output_configs().unwrap();
        let output_config = output_supported_config
            .next()
            .unwrap()
            .with_max_sample_rate();
        
        let output_sample_format = output_config.sample_format();

        let output_mic_data_arc_f32 = Arc::clone(&self.current_mic_data_f32);
        let output_mic_data_arc_i16 = Arc::clone(&self.current_mic_data_i16);

        let decoder = Arc::new(Mutex::new(Decoder::new(SampleRate::Hz48000, Channels::Stereo).unwrap()));

        thread::spawn(move || {
            loop {
                match event_receiver.recv(){
                    Ok(event) => {
                        match event {
                            SocketEvent::Packet(packet) => {
                                godot_print!("received packet");
                                let msg = packet.payload();
                                let recv_buf = msg.to_vec();

                                let packet_id = u32::from_le_bytes(
                                    recv_buf[0..4].try_into().unwrap(),
                                );
                                godot_print!("Packet id: {}", packet_id);
                                let encoded_buffer = recv_buf[4..].to_vec();

                                let packed_encoded =
                                    Packet::try_from(&encoded_buffer).unwrap();

                                match output_sample_format {
                                    SampleFormat::F32 => {
                                        let mut decoded_vec: Vec<f32> = vec![0.0; 8192];
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
                                                    output_mic_data_arc_f32.lock().unwrap();
                                                *mic_data =
                                                    decoded_vec[0..size].to_vec();
                                            }
                                            Err(e) => {
                                                godot_print!("error: {}", e);
                                            }
                                        }
                                    },
                                    SampleFormat::I16 => {
                                        let mut decoded_vec: Vec<i16> = vec![0; 10240];
                                        let mut_decoded =
                                            MutSignals::try_from(&mut decoded_vec).unwrap();
                                        let decoded_size =
                                            decoder.lock().unwrap().decode(
                                                Some(packed_encoded),
                                                mut_decoded,
                                                false,
                                            );
                                        match decoded_size {
                                            Ok(size) => {
                                                let mut mic_data =
                                                    output_mic_data_arc_i16.lock().unwrap();
                                                *mic_data =
                                                    decoded_vec[0..size].to_vec();
                                            }
                                            Err(e) => {
                                                godot_print!("decoding error: {}", e);
                                            }
                                        }
                                    },
                                    SampleFormat::U16 => {}
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
                    Err(err) => {
                        godot_print!("Recv error: {}", err);
                    }
                }
            }
        });

        


        let output_stream = 
        match output_config.sample_format() {
            SampleFormat::F32 => {
                output_device
                    .build_output_stream(
                        &output_config.config(),
                        move |data: &mut [f32], _| {
                            godot_print!("playing output f32");
                            let mut i = 0;
                            let mut mic_data = mic_data_f32.lock().unwrap();
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
                    .unwrap()
            },
            SampleFormat::I16 => {
                output_device
                    .build_output_stream(
                        &output_config.config(),
                        move |data: &mut [i16], _| {
                            // godot_print!("playing output i16");
                            let mut i = 0;
                            let mut mic_data = mic_data_i16.lock().unwrap();
                            // godot_print!("mic data size: {}", mic_data.len());
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
                    .unwrap()
            },
            SampleFormat::U16 => {
                output_device
                    .build_output_stream(
                        &output_config.config(),
                        move |data: &mut [i16], _| {
                            godot_print!("playing output u16");
                            let mut i = 0;
                            let mut mic_data = mic_data_i16.lock().unwrap();
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
                    .unwrap()
            }
        };
        
        output_stream.play().unwrap();

        godot_print!("stream set");
    }

    #[method]
    #[cfg(target_arch = "aarch64")]
    fn build_input_stream(&mut self, input_id: i32, output_id: i32){
        // let audio_server = AudioServer::godot_singleton();
        // let bus_index = audio_server.get_bus_index("Record");
        // let effect = audio_server.get_bus_effect( bus_index, 0).unwrap().cast::<AudioEffectCapture>().unwrap();
        // thread::spawn(move || {
        //     let safe_effect = unsafe {effect.assume_safe()};
        //     let data = safe_effect.get_buffer(safe_effect.get_frames_available()).read().to_vec();
            
        // });
        use aaudio::AAudioStreamBuilder;

        let input_builder = AAudioStreamBuilder::new()
            .unwrap()
            .set_sample_rate(48_000)
            .set_channel_count(1)
            .set_format(aaudio::Format::I16)
            .set_performance_mode(aaudio::PerformanceMode::LowLatency)
            .set_usage(aaudio::Usage::VoiceCommunication)
            .set_buffer_capacity_in_frames(960)
            .set_sharing_mode(aaudio::SharingMode::Shared)
            .set_direction(aaudio::Direction::Input)
            .set_device_id(input_id);
        
        let output_builder = AAudioStreamBuilder::new()
            .unwrap()
            .set_sample_rate(48_000)
            .set_channel_count(1)
            .set_format(aaudio::Format::I16)
            .set_performance_mode(aaudio::PerformanceMode::LowLatency)
            .set_usage(aaudio::Usage::VoiceCommunication)
            .set_buffer_capacity_in_frames(960)
            .set_sharing_mode(aaudio::SharingMode::Shared)
            .set_direction(aaudio::Direction::Output)
            .set_device_id(output_id);
    
        match input_builder.open_stream() {
            Ok(mut input_stream) => {
                match input_stream.request_start() {
                    Ok(_) => {
                        match output_builder.open_stream(){
                            Ok(mut output_stream) => {
                                match output_stream.request_start() {
                                    Ok(_) => {
                                        thread::spawn(move || {
                                            let encoder = Encoder::new(SampleRate::Hz48000, Channels::Mono, audiopus::Application::Voip).unwrap();
                                            let mut decoder = Decoder::new(SampleRate::Hz48000, Channels::Mono).unwrap();
                                            loop{
                                                let mut buffer: [u8; 2048] = [0; 2048];
                                                let record = input_stream.read(&mut buffer, 960, 1_000_000_000);
                                                match record {
                                                    Ok(size) => {
                                                        let buffer_i16 = unsafe {
                                                            std::slice::from_raw_parts_mut(
                                                                buffer.as_ptr() as *mut i16,
                                                                960
                                                            )
                                                        };
                                                        let mut encoded_data:[u8; 2048] = [0; 2048];
                                                        // let mut sliced_buffer = &buffer[..size as usize];
                                                        godot_print!("raw size {}", size);
                                                        // let mut buffer_i16: Vec<i16> = Vec::new();
                                                        // let mut i: usize = 0;
                                                        // while(i < size as usize){
                                                        //     // let bytes = [sliced_buffer[i], sliced_buffer[i+1]];
                                                        //     // let byte_i16 = i16::from_le_bytes(bytes);
                                                        //     // buffer_i16.push(byte_i16);
                                                        //     // i = i + 2;
                                                        //     let value = (sliced_buffer[i] as i16 - 128) * 256;
                                                        //     buffer_i16.push(value);
                                                        //     i += 1;
                                                        // }
                                                        match encoder.encode(&buffer_i16, &mut encoded_data){
                                                            Ok(size) => {
                                                                godot_print!("Encoded size {}", size);
                                                                let sliced_buffer = &encoded_data[..size];
                                                                let packet_encoded = Packet::try_from(sliced_buffer).unwrap();
                                                                let mut decoded_vec = vec![0 as i16; 4096];
                                                                let decoded_buffer = MutSignals::try_from(&mut decoded_vec).unwrap();
                                                                match decoder.decode(Some(packet_encoded), decoded_buffer, false){
                                                                    Ok(size) => {
                                                                        godot_print!("Decoded size {}", size);
                                                                        let sliced_buffer = &decoded_vec[..size];
                                                                        let buffer_u8 = unsafe {
                                                                            std::slice::from_raw_parts_mut(
                                                                                sliced_buffer.as_ptr() as *mut u8
                                                                            , size)
                                                                        };
                                                                        match output_stream.write(buffer_u8, 960, 1_000_000_000) {
                                                                            Ok(size) => {
                                                                                godot_print!("Bytes written: {}", size);
                                                                            },
                                                                            Err(err) => {
                                                                                godot_print!("Error writing bytes: {}", err);
                                                                            }
                                                                        }
                                                                    },
                                                                    Err(err) => {
                                                                        godot_print!("Decoding error: {}", err);
                                                                    }
                                                                }
                                                            },
                                                            Err(err) => {
                                                                godot_print!("Encoding error: {}", err);
                                                            }
                                                        }

                                                        // match output_stream.write(sliced_buffer, 960, 1_000_000_000) {
                                                        //     Ok(size) => {
                                                        //         godot_print!("Bytes written: {}", size);
                                                        //     },
                                                        //     Err(err) => {
                                                        //         godot_print!("Error writing bytes: {}", err);
                                                        //     }
                                                        // }
                                                        // godot_print!("recorded size: {}", size);
                                                    },
                                                    Err(err) => {
                                                        godot_print!("{}", err);
                                                    }
                                                }
                                            }
                                        });
                                    },
                                    Err(err) => {
                                        godot_print!("COuld not start output stream: {}", err);
                                    }
                                }
                            },
                            Err(err) => {
                                godot_print!("COuld not open output stream: {}", err);
                            }
                        }
                    },
                    Err(err) => {
                        godot_print!("start err: {}", err);
                    }
                }
            },
            Err(err) => {
                godot_print!("open stream err: {}", err);
            }
        }


        
        
    }
}
