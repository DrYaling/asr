//! aes crypto mod
#![allow(unused)]
///encrypt data
pub fn encrypt(mut bytes: Vec<u8>, _input: i32) -> anyhow::Result<Vec<u8>>{
    //TO BE DONE
    Ok(bytes)
}
///decrypt data
pub fn decrypt(mut bytes: Vec<u8>, _input: i32) -> anyhow::Result<Vec<u8>>{
    //TO BE DONE
    Ok(bytes)
}
#[cfg(test)]
#[test]
fn aes_test(){
    use aes::{Aes128, Block, ParBlocks};
    use aes::cipher::{
        BlockEncrypt, BlockDecrypt, NewBlockCipher,
        generic_array::GenericArray,
    };

    let key = GenericArray::from_slice(&[0u8; 16]);
    let mut block = Block::default();
    let mut block8 = ParBlocks::default();

    // Initialize cipher
    let cipher = Aes128::new(&key);

    let block_copy = block.clone();

    println!("block_copy {:?}", block_copy);
    // Encrypt block in-place
    cipher.encrypt_block(&mut block);
    println!("encrypt_block {:?}", block);
    // And decrypt it back
    cipher.decrypt_block(&mut block);
    println!("decrypt_block {:?}", block);
    

    // We can encrypt 8 blocks simultaneously using
    // instruction-level parallelism
    let block8_copy = block8.clone();
    println!("block8_copy {:?}", block8_copy);
    cipher.encrypt_par_blocks(&mut block8);
    println!("encrypt_par_blocks {:?}", block);
    cipher.decrypt_par_blocks(&mut block8);
    println!("decrypt_par_blocks {:?}", block);
    assert_eq!(block8, block8_copy);
    let mut buff = Vec::<u8>::new();
    //100m buffer aes and des
    for elem in 0..1024*1024*100 {
        buff.push((elem & 0xf) as u8);
    }
    let buff_copy = buff.clone();
    println!("buffer {:?}", buff.len());
    let time = std::time::Instant::now();
    let encrypt0 = encrypt(buff, 0).unwrap();
    println!("encrypt {:?}, cost {}", encrypt0.len(), time.elapsed().as_millis());
    let time = std::time::Instant::now();
    let decrypt0 = decrypt(encrypt0, 0).unwrap();
    println!("decrypt0 {:?}, cost {}", decrypt0.len(), time.elapsed().as_millis());

    let mut c0 = buff_copy.chunks(16);
    let mut c1 = decrypt0.chunks(16);
    let mut index = 0;
    loop {
        let t = (c0.next(), c1.next());
        index += 16;
        if let (Some(i1), Some(i2)) = &t{
            assert_eq!(i1, i2, "at chunk index {}", index);
        }
        else{
            println!("t is {:?}", t);
            break;
        }
    }
    //assert_eq!(decrypt, buff_copy);

}