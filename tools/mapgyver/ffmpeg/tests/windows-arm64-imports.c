#include <stdio.h>

#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libavutil/hwcontext.h>
#include <libswresample/swresample.h>
#include <libswscale/swscale.h>

int main(void) {
    if ((avcodec_version() >> 16) != LIBAVCODEC_VERSION_MAJOR ||
        (avformat_version() >> 16) != LIBAVFORMAT_VERSION_MAJOR ||
        (avutil_version() >> 16) != LIBAVUTIL_VERSION_MAJOR ||
        (swresample_version() >> 16) != LIBSWRESAMPLE_VERSION_MAJOR ||
        (swscale_version() >> 16) != LIBSWSCALE_VERSION_MAJOR) {
        fputs("FFmpeg header and DLL major versions differ\n", stderr);
        return 1;
    }
    if (!avcodec_find_decoder_by_name("h264") ||
        av_hwdevice_find_type_by_name("d3d11va") != AV_HWDEVICE_TYPE_D3D11VA ||
        av_hwdevice_find_type_by_name("dxva2") != AV_HWDEVICE_TYPE_DXVA2) {
        fputs("H.264 or a Windows hardware decode API is missing\n", stderr);
        return 1;
    }
    puts("ARM64 FFmpeg import libraries, DLLs and decode APIs verified");
    return 0;
}
