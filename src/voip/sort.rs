use super::VoicePacket;

pub fn quick_sort(mut voice_packets: Vec<VoicePacket>) -> Vec<VoicePacket>{
    
    if voice_packets.len() == 0 {
        return voice_packets;
    }

    let mut pivot = voice_packets.len();
    let mut i = 0;
    let mut j = 1;

    while j != pivot {
        if voice_packets[j - 1].id < voice_packets[pivot - 1].id {
            i += 1;
            let swapping_buffer = voice_packets[j - 1].clone();
            voice_packets[j - 1] = voice_packets[i - 1].clone();
                voice_packets[i - 1] = swapping_buffer;
        }
        j += 1;
    }
    let saved_pivot = voice_packets[pivot - 1].clone();

    voice_packets.remove(pivot - 1);

    pivot = i;

    voice_packets.insert(pivot, saved_pivot);

    if voice_packets.len() < 3 {
        return voice_packets;
    }

    let split = voice_packets.split_at(pivot);

    let left = quick_sort(split.0.to_vec());
    let mut right = quick_sort(split.1.to_vec());

    let mut new_pool = left;
    new_pool.append(&mut right);

    new_pool
}