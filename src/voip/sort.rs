pub fn quick_sort(mut pool: Vec<Vec<u8>>) -> Vec<Vec<u8>>{
    if pool.len() == 0 {
        return pool;
    }

    let mut pivot = pool.len();
    let mut i = 0;
    let mut j = 1;

    while j != pivot {
        if get_id(pool[j - 1].clone()) < get_id(pool[pivot - 1].clone()) {
            i += 1;
            let swapping_buffer = pool[j - 1].clone();
            pool[j - 1] = pool[i - 1].clone();
                pool[i - 1] = swapping_buffer;
        }
        j += 1;
    }
    let saved_pivot = pool[pivot - 1].clone();

    pool.remove(pivot - 1);

    pivot = i;

    pool.insert(pivot, saved_pivot);

    if pool.len() < 3 {
        return pool;
    }

    let split = pool.split_at(pivot);

    let left = quick_sort(split.0.to_vec());
    let mut right = quick_sort(split.1.to_vec());

    let mut new_pool = left;
    new_pool.append(&mut right);

    new_pool
}

fn get_id(buffer: Vec<u8>) -> u32 {
    u32::from_le_bytes(
        buffer[..4].try_into().unwrap(),
    )
}