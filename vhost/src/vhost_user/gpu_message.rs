//! Implementation parts of the protocol on the socket from VHOST_USER_SET_GPU_SOCKET
//! see: https://www.qemu.org/docs/master/interop/vhost-user-gpu.html

use crate::vhost_user::header::MsgHeader;
use crate::vhost_user::message::{
    enum_value, BackendReq, Req, VhostUserMsgValidator, MAX_MSG_SIZE,
};
use crate::vhost_user::Error;
use std::fmt::Debug;
use std::marker::PhantomData;
use vm_memory::ByteValued;

enum_value! {
    /// Type of requests sending from gpu backends to gpu frontends.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
    #[allow(non_camel_case_types)]
    pub enum GpuBackendReq: u32 {
        /// Get the supported protocol features bitmask.
        GET_PROTOCOL_FEATURES = 1,
        /// Enable protocol features using a bitmask.
        SET_PROTOCOL_FEATURES,
        /// Get the preferred display configuration.
        GET_DISPLAY_INFO,
        /// Set/show the cursor position.
        CURSOR_POS,
        /// Set/hide the cursor.
        CURSOR_POS_HIDE,
        /// Set the scanout resolution.
        /// To disable a scanout, the dimensions width/height are set to 0.
        SCANOUT,
        /// Update the scanout content. The data payload contains the graphical bits.
        /// The display should be flushed and presented.
        UPDATE,
        /// Set the scanout resolution/configuration, and share a DMABUF file descriptor for the scanout content,
        /// which is passed as ancillary data.
        /// To disable a scanout, the dimensions width/height are set to 0, there is no file descriptor passed.
        DMABUF_SCANOUT,
        /// The display should be flushed and presented according to updated region from VhostUserGpuUpdate.
        // Note: there is no data payload, since the scanout is shared thanks to DMABUF,
        // that must have been set previously with VHOST_USER_GPU_DMABUF_SCANOUT.
        DMABUF_UPDATE,
        /// Retrieve the EDID data for a given scanout.
        /// This message requires the VHOST_USER_GPU_PROTOCOL_F_EDID protocol feature to be supported.
        GET_EDID,
        /// Same as VHOST_USER_GPU_DMABUF_SCANOUT, but also sends the dmabuf modifiers appended to the message,
        /// which were not provided in the other message.
        /// This message requires the VHOST_USER_GPU_PROTOCOL_F_DMABUF2 protocol feature to be supported.
        VHOST_USER_GPU_DMABUF_SCANOUT2,
    }
}

impl Req for BackendReq {}

// Bit mask for common message flags.
bitflags! {
    /// Common message flags for vhost-user requests and replies.
    pub struct VhostUserGpuHeaderFlag: u32 {
        /// Mark message as reply.
        const REPLY = 0x4;
    }
}

/// A vhost-user message consists of 3 header fields and an optional payload. All numbers are in the
/// machine native byte order.
#[repr(C, packed)]
#[derive(Copy)]
pub struct VhostUserGpuMsgHeader<R: Req> {
    request: u32,
    flags: u32,
    size: u32,
    _r: PhantomData<R>,
}

impl<R: Req> Debug for VhostUserGpuMsgHeader<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VhostUserMsgHeader")
            .field("request", &{ self.request })
            .field("flags", &{ self.flags })
            .field("size", &{ self.size })
            .finish()
    }
}

impl<R: Req> Clone for VhostUserGpuMsgHeader<R> {
    fn clone(&self) -> VhostUserGpuMsgHeader<R> {
        *self
    }
}

impl<R: Req> PartialEq for VhostUserGpuMsgHeader<R> {
    fn eq(&self, other: &Self) -> bool {
        self.request == other.request && self.flags == other.flags && self.size == other.size
    }
}

#[allow(dead_code)]
impl<R: Req> VhostUserGpuMsgHeader<R> {
    /// Create a new instance of `VhostUserMsgHeader`.
    pub fn new(request: R, flags: u32, size: u32) -> Self {
        // Default to protocol version 1
        let fl = (flags & VhostUserGpuHeaderFlag::ALL_FLAGS.bits()) | 0x1;
        VhostUserGpuMsgHeader {
            request: request.into(),
            flags: fl,
            size,
            _r: PhantomData,
        }
    }

    /// Get message type.
    pub fn get_code(&self) -> crate::vhost_user::Result<R> {
        R::try_from(self.request).map_err(|_| Error::InvalidMessage)
    }

    /// Check whether it's a reply message.
    pub fn is_reply(&self) -> bool {
        (self.flags & VhostUserGpuHeaderFlag::REPLY.bits()) != 0
    }

    /// Mark message as reply.
    pub fn set_reply(&mut self, is_reply: bool) {
        if is_reply {
            self.flags |= VhostUserGpuHeaderFlag::REPLY.bits();
        } else {
            self.flags &= !VhostUserGpuHeaderFlag::REPLY.bits();
        }
    }

    /// Check whether reply for this message is requested.
    pub fn is_need_reply(&self) -> bool {
        (self.flags & VhostUserGpuHeaderFlag::NEED_REPLY.bits()) != 0
    }

    /// Check whether it's the reply message for the request `req`.
    pub fn is_reply_for(&self, req: &VhostUserGpuMsgHeader<R>) -> bool {
        if let (Ok(code1), Ok(code2)) = (self.get_code(), req.get_code()) {
            self.is_reply() && !req.is_reply() && code1 == code2
        } else {
            false
        }
    }

    /// Get message size.
    pub fn get_size(&self) -> u32 {
        self.size
    }

    /// Set message size.
    pub fn set_size(&mut self, size: u32) {
        self.size = size;
    }
}

impl<R: Req> Default for VhostUserGpuMsgHeader<R> {
    fn default() -> Self {
        VhostUserGpuMsgHeader {
            request: 0,
            flags: 0x1,
            size: 0,
            _r: PhantomData,
        }
    }
}

// SAFETY: Safe because all fields of VhostUserMsgHeader are POD.
unsafe impl<R: Req> ByteValued for VhostUserGpuMsgHeader<R> {}

impl<T: Req> VhostUserMsgValidator for VhostUserGpuMsgHeader<T> {
    #[allow(clippy::if_same_then_else)]
    fn is_valid(&self) -> bool {
        self.get_code().is_ok()
    }
}

impl<R: Req> MsgHeader for VhostUserGpuMsgHeader<R> {
    type Request = R;
}