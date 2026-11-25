// Access the API-specific public FFmpeg context through its owning SDK headers.
#define COBJMACROS
#include <libavutil/hwcontext.h>
#include <libavutil/hwcontext_d3d11va.h>

int32_t golden_video_init_decoder_device(AVBufferRef *reference, ID3D11Device *device)
{
    AVHWDeviceContext *context = (AVHWDeviceContext *)reference->data;
    AVD3D11VADeviceContext *native = context->hwctx;
    ID3D11Device_AddRef(device);
    native->device = device;
    return av_hwdevice_ctx_init(reference);
}

int32_t golden_video_decoder_device_status(const AVBufferRef *reference)
{
    if (!reference || !reference->data)
        return E_POINTER;
    const AVHWDeviceContext *context = (const AVHWDeviceContext *)reference->data;
    if (context->type != AV_HWDEVICE_TYPE_D3D11VA || !context->hwctx)
        return E_NOINTERFACE;
    const AVD3D11VADeviceContext *native = context->hwctx;
    if (!native->device)
        return E_POINTER;
    return ID3D11Device_GetDeviceRemovedReason(native->device);
}
