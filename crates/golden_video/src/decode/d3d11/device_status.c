// Access the API-specific public FFmpeg context through its owning SDK headers.
#define COBJMACROS
#include <libavutil/hwcontext.h>
#include <libavutil/hwcontext_d3d11va.h>

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
