use audiopus::coder::{Decoder, Encoder};
use audiopus::packet::Packet;
use audiopus::{ MutSignals, SampleRate};
use gdnative::api::{AudioStreamPlayer, AudioStreamGeneratorPlayback};
use gdnative::prelude::*;
use laminar::{Socket, Packet as UDPPacket, SocketEvent};

use std::convert::{TryFrom, TryInto};
use std::sync::{Arc, Mutex};
use std::{thread, time};

mod sort;
use sort::quick_sort;

#[derive(NativeClass)]
#[inherit(Node)]
#[register_with(Self::register_signals)]
pub struct GodotVoip {
    input_stream: Option<cpal::Stream>,
    remote_address: Arc<Mutex<String>>
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
            input_stream: None,
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
    fn build_input_stream(&mut self, player: Ref<AudioStreamPlayer>, _id: i32, _output_id: i32) {
        let playback = unsafe {player.assume_safe().get_stream_playback().unwrap().try_cast::<AudioStreamGeneratorPlayback>().unwrap()};
        unsafe {player.assume_safe().play(0.0)};
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

        let encoder = Encoder::new(
            SampleRate::Hz48000,
            audiopus::Channels::Mono,
            audiopus::Application::Voip,
        )
        .unwrap();
        // encoder.set_max_bandwidth(audiopus::Bandwidth::Narrowband).unwrap();
        // encoder.set_force_channels(Channels::Mono);
        // let encoder_arc = Arc::new(Mutex::new(encoder));

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
                        let e_size = encoder.encode_float(data, &mut encoded);
                        match e_size {
                            Ok(size) => {

                                let mut last_id = last_sent_packet_id.lock().unwrap();
                                let mut message = last_id.clone().to_le_bytes().to_vec();
                                message.extend_from_slice(&encoded[0..size]);

                                if last_id.clone() == u32::MAX {
                                    *last_id = 0;
                                }else{
                                    *last_id = last_id.clone() + 1;
                                }

                                
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
                        
                        let e_size = encoder.encode(data, &mut encoded);
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
                        let e_size = encoder.encode(data, &mut encoded);
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
        self.input_stream = Some(input_stream);

        let mut decoder = Decoder::new(SampleRate::Hz48000, audiopus::Channels::Mono).unwrap();

        let mut next_voice_pool = Arc::new(Mutex::new(Vec::new()));

        let mut next_voice_pool_push = Arc::clone(&next_voice_pool);
        
        thread::spawn(move || {
            loop {
                match event_receiver.recv(){
                    Ok(event) => {
                        match event {
                            SocketEvent::Packet(packet) => {
                                godot_print!("received packet");
                                teste();
                                let msg = packet.payload();
                                let recv_buf = msg.to_vec();
                                next_voice_pool_push.lock().unwrap().push(recv_buf);

                                // let packet_id = u32::from_le_bytes(
                                //     recv_buf[0..4].try_into().unwrap(),
                                // );
                                // godot_print!("Packet id: {}", packet_id);
                                // let encoded_buffer = recv_buf[4..].to_vec();

                                // let packet_encoded = Packet::try_from(&encoded_buffer).unwrap();

                                // let mut buffer: Vec<f32> = vec![0.0; 4096];
                                // let mut mut_signal_buffer = MutSignals::try_from(&mut buffer).unwrap();
                                // match decoder.decode_float(Some(packet_encoded), mut_signal_buffer, false){
                                //     Ok(size) => {
                                //         let buf = &buffer[..size];
                                //         let mut vector_buffer = Vec::new();
                                //         for b in buf {
                                //             vector_buffer.push(Vector2::new(b.clone() as f32, b.clone() as f32));
                                //         }
                                //         unsafe{playback.assume_safe().push_buffer(PoolArray::from_vec(vector_buffer))};
                                //     },
                                //     Err(err) => {
                                //         godot_print!("{}", err);
                                //     }
                                // }
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

        let mut next_voice_pool_clean = Arc::clone(&next_voice_pool);

        thread::spawn(move || {
            let mut i: u8 = 0;
            let mut current_voice_pool: Vec<Vec<u8>> = Vec::new();
            loop {
                i += 1;
                if i > 25{
                    //TODO: get next voice pool and order it
                    current_voice_pool = quick_sort(next_voice_pool_clean.lock().unwrap().to_vec());
                    next_voice_pool_clean.lock().unwrap().clear();
                    i = 0;
                }

                if current_voice_pool.len() > 0 {
                    let recv_buf = &current_voice_pool[0];

                    let encoded_buffer = recv_buf[4..].to_vec();

                    let packet_encoded = Packet::try_from(&encoded_buffer).unwrap();

                    let mut buffer: Vec<f32> = vec![0.0; 4096];
                    let mut mut_signal_buffer = MutSignals::try_from(&mut buffer).unwrap();
                    match decoder.decode_float(Some(packet_encoded), mut_signal_buffer, false){
                        Ok(size) => {
                            let buf = &buffer[..size];
                            let mut vector_buffer = Vec::new();
                            for b in buf {
                                vector_buffer.push(Vector2::new(b.clone() as f32, b.clone() as f32));
                            }
                            unsafe{playback.assume_safe().push_buffer(PoolArray::from_vec(vector_buffer))};
                        },
                        Err(err) => {
                            godot_print!("{}", err);
                        }
                    }
                    current_voice_pool.remove(0);
                }

                //sleep for 5 ms
                thread::sleep(time::Duration::from_millis(5));
            }
        });

        godot_print!("stream set");
    }

    #[method]
    #[cfg(target_arch = "aarch64")]
    fn build_input_stream(&mut self, player: Ref<AudioStreamPlayer>, input_id: i32, output_id: i32){
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

fn teste(){
    godot_print!("teste foi um sucesso!");
}