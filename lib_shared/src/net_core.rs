use std::process::abort;
use std::io::{Write, Read};
use bytes::BufMut;
use bytes::buf::UninitSlice;
/// ---------------------------------------------------------------------------------------------------------------------------------
/// | len 2 | cbCheckCode 1 | msg type 1 | main cmd 2 | sub cmd 2 | rpc call id 4(该字段根据msg type是否包含MSG_TYPE_RPC可选) | data |
/// ---------------------------------------------------------------------------------------------------------------------------------
#[repr(C)]
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Header{
    ///header size in bytes
    h_size: u32,
    flag: u8,
    crc: u8,
    pub size: u32,
    pub squence: u32,
    pub code: u32,
}

///default byte buffer size
pub const DEFAULT_BUFFER_SIZE: usize = 8*8192;
const HEADER_SIZE: u32 = 8;
////////////////////////////////
/// header 0-4 flag,
///////////////////////////////
#[allow(unused)]
impl Header{
    pub fn new(mut main_code: u16, sub_code: u16, size: u32, compress: bool, rpc_seq: Option<u32>) -> Self{
        //缺省主协议号
        if main_code == 0 && sub_code != 1{
            main_code = 101;
        }
        let mut flag = if_else!(compress, 0x8, 0) as u8;
        // println!("flag {}", flag);
        let squence = rpc_seq.unwrap_or_default();
        flag |= if_else!(squence > 0,1,0);
        let crc: u8 = 0;
        Self{
            size,flag, crc, squence, code: ((main_code as u32 ) | (sub_code as u32) << 16u32),
            h_size: if_else!(squence > 0, HEADER_SIZE + 4, HEADER_SIZE)
        }
    }
    #[inline]
    pub fn compressed(&self) -> bool{
        self.flag & 0x8 != 0
    }
    #[inline]
    pub fn sub_code(&self) -> u16{
        (self.code &0xffff ) as u16
    }
    #[inline]
    pub fn code(&self) -> u16{
        ((self.code & 0xffff0000) >> 16) as u16
    }
    #[inline]
    pub fn size(&self) -> u32{
        self.size
    }
    pub fn serialize(&self) -> Vec<u8>{
        let mut ret = Vec::with_capacity((HEADER_SIZE+4 + self.size >> 16u32) as usize);
        // let size = self.size | ((self.flag as u32) << 24u32);
        let mut size: u16 = 0;
        if self.squence > 0 {
            size = self.size as u16+ 10;            
        } else {
            size = self.size as u16 + 6;
        }
        let size = size.to_le_bytes();
        // let crc = self.crc.to_be_bytes();
        ret.extend(&size);
        ret.push(0);
        ret.push(self.flag);
        // ret.extend(&crc);
        let code = self.code.to_le_bytes();
        ret.extend(&code);
        if self.squence > 0 {
            let seq = self.squence.to_le_bytes();
            ret.extend(&seq);
        }
        ret
    }
    #[inline]
    pub fn squence(&self) -> u32{
        self.squence
    }
    #[inline]
    pub fn get_header_size(&self) -> usize{
        self.h_size as usize
    }
    #[inline]
    ///check if packet is rpc packet
    pub fn rpc(bytes: &[u8]) ->bool{
        if bytes.len() <= 3 {
            false
        }
        else{
            let flag: u8 = u8::from_le_bytes([bytes[3]]);
            (flag & 0x1) != 0
        }
    }
    ///calculate header size
    pub fn header_size(header: &[u8]) -> usize{
        (if Self::rpc(header){
            HEADER_SIZE + 4
        }
        else{
            HEADER_SIZE
        }) as usize
    }

}
impl From<&[u8]> for Header{
    fn from(t: &[u8]) -> Self {
        assert!(t.len() >= 8);
        let arr1: [u8;4] = [t[0],t[1],0,0];
        let mut size: u32 = u32::from_le_bytes(arr1) - 6;
        let flag: u8 = u8::from_le_bytes([t[3]]);
        let crc: u8 = u8::from_le_bytes([t[2]]);
        let main_code: u16 = u16::from_le_bytes([t[6],t[7]]);
        let sub_code: u16 = u16::from_le_bytes([t[4],t[5]]);
        let code: u32 = main_code as u32 | (sub_code as u32) << 16u32;
        let squence: u32 = if (flag & 0x1) != 0{
            size -= 4;
            u32::from_le_bytes([t[8],t[9],t[10],t[11]])
        }
        else{
            0
        };
        if size > 10 * 1024 * 1024{
            log_error!("msg size of {}:{} error: {}(longger than 10M), header buffer {:?}", main_code, sub_code, size, t);
            size = 0;
        }
        //println!("pack flag {}, size {}, opcode {}, crc {}, squence {}, header_size {}", flag,size,code,crc,squence,Header::header_size(t));
        Header{
            // size,flag,squence, crc, code, h_size: if_else!(squence > 0, HEADER_SIZE + 4, HEADER_SIZE)
            size, flag, crc, squence, code, h_size: if_else!(squence > 0, HEADER_SIZE + 4, HEADER_SIZE)
        }
    }
}
#[allow(unused)]
pub struct ByteBuffer{
    buffer: Vec<u8>,
    wpos: usize,
    rpos: usize,
}
#[allow(unused)]
impl ByteBuffer{
    pub fn new(mut size: usize) -> Self{
        if size > 10 * 1024 * 1024{
            log_error!("create byte buffer fail, msg len error {}", size);
            size = 1024;
        }
        Self{
            buffer: Vec::with_capacity(size),
            wpos: 0,rpos: 0,
        }
    }
    pub fn reset(&mut self){
        // self.buffer.clear();
        self.wpos = 0;
        self.rpos = 0;
    }
    pub fn trim(&mut self){
        if self.rpos > 0{
            self.buffer.copy_within(self.rpos..self.wpos, 0);
            self.wpos -= self.rpos;
            self.rpos = 0;
        }
    }
    ///trim if read pos is half size of buffer size
    pub fn trim_step(&mut self){
        if self.rpos > self.buffer.len() / 2{
            self.trim()
        }
    }
    #[inline]
    pub fn size(&self) -> usize{
        self.wpos - self.rpos
    }
    ///读取数据，如果读取的长度不足，则读取失败
    pub fn read(&mut self, buf: &mut [u8], size: usize) -> std::io::Result<usize>{
        if self.size() < size{
            Err(std::io::ErrorKind::WouldBlock.into())
        }
        else{
            // println!("buf size {},size {}",buf.len(),size);
            buf[0..size].copy_from_slice(&self.buffer[self.rpos..self.rpos+size]);
            self.rpos += size;
            Ok(size)
        }
    }
    #[inline]
    fn ensure_size(&mut self,size: usize) -> std::io::Result<()>{
        if self.buffer.capacity() > self.wpos + size{
            self.buffer.resize_with(size + self.wpos,|| Default::default());
            Ok(())
        }
        else{
            if self.buffer.len() + size > usize::MAX/2{
                log_error!("error memory management!");
                abort();
            }
            self.buffer.reserve(self.wpos + size*2);
            self.buffer.resize_with(size + self.wpos,|| Default::default());
            Ok(())
        }
    }
    ///写入数据
    pub fn write(&mut self, buf: &[u8], size: usize){
        self.ensure_size(size);
        self.buffer[self.wpos..self.wpos+size].copy_from_slice(&buf[0..size]);
        self.wpos += size;
    }
    #[inline]
    pub fn as_slice(&self) -> &[u8]{
        &self.buffer.as_slice()[self.rpos..self.wpos]
    }
    ///read complete operation
    pub fn read_complete(&mut self, size: usize) -> std::io::Result<()>{
        if self.rpos + size <= self.wpos{
            self.rpos += size;
            Ok(())
        }
        else{
            Err(std::io::ErrorKind::InvalidData.into())
        }
    }
    pub fn write_complete(&mut self, size: usize){
        if self.wpos + size <= self.buffer.len(){
            self.wpos += size;
        }
    }
    ///user shoud know that: the returned buffer contains unuse bytes outbounds of wpos and rpos
    #[inline]
    pub fn into_vec(self) -> Vec<u8>{
        self.buffer
    }
}
impl Read for ByteBuffer{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.read(buf, self.size()).map_err(|_| std::io::ErrorKind::UnexpectedEof.into())
    }
}
impl Write for ByteBuffer{
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // println!("write byte buffer {}",buf.len());
        self.write(buf, buf.len());
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
unsafe impl bytes::BufMut for ByteBuffer{
    fn remaining_mut(&self) -> usize {
        usize::MAX - self.wpos
    }

    unsafe fn advance_mut(&mut self, cnt: usize) {
        let len = self.wpos;
        let remaining = self.buffer.capacity() - len;
        assert!(
            cnt <= remaining,
            "cannot advance past `remaining_mut`: {:?} <= {:?}",
            cnt,
            remaining
        );
        self.buffer.set_len(len + cnt);
    }

    fn chunk_mut(&mut self) -> &mut bytes::buf::UninitSlice {
        if self.buffer.capacity() == self.wpos {
            self.buffer.reserve(64); // Grow the vec
        }

        let cap = self.buffer.capacity();
        let len = self.wpos;

        let ptr = self.buffer.as_mut_ptr();
        unsafe { &mut UninitSlice::from_raw_parts_mut(ptr, cap)[len..] }
    }
}
impl bytes::Buf for ByteBuffer{
    fn remaining(&self) -> usize {
        self.size()
    }

    fn chunk(&self) -> &[u8] {
        self.as_slice()
    }

    fn advance(&mut self, cnt: usize) {
        self.read_complete(cnt).ok();
    }
}
#[allow(unused)]
#[derive(Debug)]
pub struct PackBuffer{
    header: Header,
    buffer: Vec<u8>,
}
#[allow(unused)]
impl PackBuffer{
    #[inline]
    pub fn header(&self)->&Header{
        &self.header
    }
    #[inline]
    pub fn from_header(header: Header) -> Self{
        Self{
            buffer: Vec::with_capacity(header.size() as usize), header
        }
    }
    pub fn from(header: &[u8]) -> std::io::Result<Self>{
        if header.len() < Header::header_size(header){
            Err(std::io::ErrorKind::InvalidData.into())
        }
        else{
            let header = unpack_header(header)?;
            // println!("received header {:?}",header);
            let mut buffer = Vec::with_capacity(header.size() as usize);
            buffer.resize_with(header.size() as usize,|| 0u8);
            Ok(Self{
                buffer, header
            })
        }
    }
    ///尝试从buffer中一次性读取所有数据，如果buffer数据不足，则读取失败
    pub fn read_from_buffer(&mut self, buffer: &mut ByteBuffer) -> std::io::Result<usize>{
        buffer.read(self.buffer.as_mut_slice(), self.header.size() as usize)
    }
    pub fn read_from_bytes(&mut self, buffer: &[u8]) -> Result<usize,usize>{
       if buffer.len() <= self.header.size() as usize{
           self.buffer.put_slice(buffer);
           Ok(buffer.len())
       }
       else{
           Err(0)
       }
    }
    /// 校验
    pub fn crc(&self) -> std::io::Result<()> {
        let mut crc = self.header.crc;
        if crc == 0 {
            Ok(())
        } else {
            log_error!("fail to check received buffer");
            Err(std::io::ErrorKind::InvalidData.into())
        }
    }
    ///解包
    pub fn unpack<T: protobuf::Message>(&self) -> std::io::Result<T>{
        //println!("unpack buffer {}- {:?}",self.buffer.len(),self.buffer.as_slice());
        T::parse_from_bytes(self.buffer.as_slice()).map_err(|e|{
            log_error!("decode msg fail {:?}",e);
            std::io::Error::from(std::io::ErrorKind::InvalidData)
        })
    }
    pub fn to_buffer(mut self) -> std::io::Result<Vec<u8>>{
        let mut buffer = self.header().serialize();
        buffer[2] = get_crc(&buffer, &self.buffer);
        buffer.append(&mut self.buffer);
        Ok(buffer)
    }
    pub fn msg_buffer(self) -> Vec<u8>{
        self.buffer
    }
    pub fn print(&self){
        log_info!("{:?}",self);
    }
}
///打包二进制
pub fn pack<'a, T: protobuf::Message>(code: u16, sub_code: u16, data: T, rpc: Option<u32>) -> std::io::Result<Vec<u8>>{    
    //todo compute pack size
    let data_size = data.compute_size();
    let header = Header::new(code, sub_code, data_size, data.get_cached_size() > 32*1024,rpc);
    let mut bytes = header.serialize();
    let mut body = data.write_to_bytes().map_err(|e|{
        log_error!("fail to pack data size {:?}",e);
        std::io::Error::from(std::io::ErrorKind::InvalidData)
    })?;
    bytes.append(&mut body);
    Ok(bytes)
}
///打包二进制
pub fn pack_box<'a>(code: u16, sub_code: u16, data: Box<dyn protobuf::Message>, rpc: Option<u32>) -> std::io::Result<Vec<u8>>{    
    //todo compute pack size
    let data_size = data.compute_size();
    let header = Header::new(code, sub_code, data_size, data.get_cached_size() > 32*1024,rpc);
    let mut bytes = header.serialize();
    
    let mut body = data.write_to_bytes().map_err(|e|{
        log_error!("fail to pack data size {:?}",e);
        std::io::Error::from(std::io::ErrorKind::InvalidData)
    })?;
    bytes.append(&mut body);
    //println!("pack opcod {}, bytes len {}, bytes {:?}",code,body.len(),&body);
    Ok(bytes)
}
#[allow(unused)]
#[inline]
///解析header
pub fn unpack_header<'a>(data: &'a [u8])  -> std::io::Result<Header>{
    if data.len() < Header::header_size(data){
        return Err(std::io::ErrorKind::InvalidData.into());
    } 
    let header = Header::from(data);
    Ok(header)
}
///打包二进制
pub fn pack_raw<'a, T: protobuf::Message>(data:& T) -> anyhow::Result<Vec<u8>>{    
    let data = data.write_to_bytes()?;
    Ok(data)
}
#[allow(unused)]
///计算check code
fn get_crc(header_buffer: &[u8], body_buffer: &[u8]) -> u8{    
    0
}