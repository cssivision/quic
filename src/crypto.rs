use bytes::BytesMut;

pub trait HeaderKey: Send {
    /// Decrypt the given packet's header
    fn decrypt(&self, pn_offset: usize, packet: &mut [u8]);
    /// Encrypt the given packet's header
    fn encrypt(&self, pn_offset: usize, packet: &mut [u8]);
    /// The sample size used for this key's algorithm
    fn sample_size(&self) -> usize;
}

pub trait PacketKey: Send {
    /// Encrypt the packet payload with the given packet number
    fn encrypt(&self, packet: u64, buf: &mut [u8], header_len: usize);
    /// Decrypt the packet payload with the given packet number
    fn decrypt(
        &self,
        packet: u64,
        header: &[u8],
        payload: &mut BytesMut,
    ) -> Result<(), CryptoError>;
    /// The length of the AEAD tag appended to packets on encryption
    fn tag_len(&self) -> usize;
    /// Maximum number of packets that may be sent using a single key
    fn confidentiality_limit(&self) -> u64;
    /// Maximum number of incoming packets that may fail decryption before the connection must be
    /// abandoned
    fn integrity_limit(&self) -> u64;
}

#[derive(Debug)]
pub struct CryptoError;

impl HeaderKey for rustls::quic::HeaderProtectionKey {
    /// Decrypt the given packet's header
    fn decrypt(&self, pn_offset: usize, packet: &mut [u8]) {
        unimplemented!()
    }

    /// Encrypt the given packet's header
    fn encrypt(&self, pn_offset: usize, packet: &mut [u8]) {
        unimplemented!()
    }

    /// The sample size used for this key's algorithm
    fn sample_size(&self) -> usize {
        unimplemented!()
    }
}

impl PacketKey for rustls::quic::PacketKey {
    /// Encrypt the packet payload with the given packet number
    fn encrypt(&self, packet: u64, buf: &mut [u8], header_len: usize) {
        let (header, payload_tag) = buf.split_at_mut(header_len);
        let (payload, tag_storage) = payload_tag.split_at_mut(payload_tag.len() - self.tag_len());
        let tag = self.encrypt_in_place(packet, header, payload).unwrap();
        tag_storage.copy_from_slice(tag.as_ref());
    }

    /// Decrypt the packet payload with the given packet number
    fn decrypt(
        &self,
        packet: u64,
        header: &[u8],
        payload: &mut BytesMut,
    ) -> Result<(), CryptoError> {
        unimplemented!()
    }

    /// The length of the AEAD tag appended to packets on encryption
    fn tag_len(&self) -> usize {
        unimplemented!()
    }

    /// Maximum number of packets that may be sent using a single key
    fn confidentiality_limit(&self) -> u64 {
        unimplemented!()
    }

    /// Maximum number of incoming packets that may fail decryption before the connection must be
    /// abandoned
    fn integrity_limit(&self) -> u64 {
        unimplemented!()
    }
}
