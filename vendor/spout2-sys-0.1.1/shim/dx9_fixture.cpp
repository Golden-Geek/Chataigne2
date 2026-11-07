// Qualification-only, independent D3D9Ex texture allocation/upload and SDK registration.
#include <d3d9.h>
#include <wrl/client.h>
#include <cstring>
#include <climits>
#include <memory>
#include "SpoutSenderNames.h"
#include "SpoutFrameCount.h"
#include "spout_shim.h"

using Microsoft::WRL::ComPtr;

namespace {
void require(HRESULT result) { if (FAILED(result)) throw result; }

struct Window {
    HWND handle = nullptr;
    ~Window() { if (handle) DestroyWindow(handle); }
};

struct Source {
    Window window;
    ComPtr<IDirect3D9Ex> d3d;
    ComPtr<IDirect3DDevice9Ex> device;
    ComPtr<IDirect3DTexture9> texture;
    ComPtr<IDirect3DSurface9> upload, target, readback;
    ComPtr<IDirect3DQuery9> completion;
    spoutSenderNames names;
    spoutFrameCount access;
    char name[256] = {};
    bool registered = false;
    bool pending = false;
    bool fenced = false;
    HANDLE shared = nullptr;

    ~Source() {
        // A failed assertion/registration cannot release an unfinished upload.
        while (pending) {
            if (!fenced) fenced = SUCCEEDED(completion->Issue(D3DISSUE_END));
            if (fenced && completion->GetData(nullptr, 0, D3DGETDATA_FLUSH) == S_OK) break;
            const HRESULT state = device->CheckDeviceState(window.handle);
            if (state == D3DERR_DEVICELOST || state == D3DERR_DEVICEHUNG || state == D3DERR_DEVICEREMOVED) break;
            Sleep(1);
        }
        if (registered) names.ReleaseSenderName(name);
    }

    void create(UINT low, LONG high, const char* requested, UINT format, UINT width, UINT height,
                const unsigned char* bytes, size_t length, UINT pitch) {
        const UINT bpp = format == D3DFMT_A16B16G16R16F ? 8 : 4;
        if (!requested || !bytes || !width || !height || width > 8 || height > 8 ||
            strlen(requested) >= sizeof(name) || pitch != width * bpp || length != pitch * height)
            throw E_INVALIDARG;
        DWORD registered_format;
        switch (format) {
        case D3DFMT_A8R8G8B8: registered_format = 21; break;
        case D3DFMT_X8R8G8B8: registered_format = 22; break;
        case D3DFMT_A16B16G16R16F: registered_format = 10; break; // DXGI RGBA16F
        case D3DFMT_A2B10G10R10: registered_format = 24; break; // DXGI RGB10A2
        default: throw E_INVALIDARG;
        }
        require(Direct3DCreate9Ex(D3D_SDK_VERSION, &d3d));
        UINT selected = UINT_MAX;
        for (UINT i = 0; i < d3d->GetAdapterCount(); ++i) {
            LUID id = {};
            require(d3d->GetAdapterLUID(i, &id));
            if (id.LowPart == low && id.HighPart == high) { selected = i; break; }
        }
        if (selected == UINT_MAX) throw HRESULT_FROM_WIN32(ERROR_NOT_FOUND);
        window.handle = CreateWindowExA(0, "STATIC", "Owned D3D9 source", WS_OVERLAPPED,
                                       0, 0, 1, 1, nullptr, nullptr, GetModuleHandle(nullptr), nullptr);
        if (!window.handle) throw HRESULT_FROM_WIN32(GetLastError());
        D3DPRESENT_PARAMETERS parameters = {};
        parameters.Windowed = TRUE;
        parameters.SwapEffect = D3DSWAPEFFECT_DISCARD;
        parameters.hDeviceWindow = window.handle;
        parameters.BackBufferWidth = parameters.BackBufferHeight = 1;
        require(d3d->CreateDeviceEx(selected, D3DDEVTYPE_HAL, window.handle,
            D3DCREATE_HARDWARE_VERTEXPROCESSING | D3DCREATE_FPU_PRESERVE,
            &parameters, nullptr, &device));
        require(device->CreateTexture(width, height, 1, D3DUSAGE_RENDERTARGET,
            static_cast<D3DFORMAT>(format), D3DPOOL_DEFAULT, &texture, &shared));
        if (!shared) throw E_HANDLE;
        require(device->CreateOffscreenPlainSurface(width, height, static_cast<D3DFORMAT>(format),
            D3DPOOL_SYSTEMMEM, &upload, nullptr));
        require(texture->GetSurfaceLevel(0, &target));
        D3DLOCKED_RECT locked = {};
        require(upload->LockRect(&locked, nullptr, 0));
        if (locked.Pitch < static_cast<INT>(pitch)) { upload->UnlockRect(); throw E_FAIL; }
        for (UINT y = 0; y < height; ++y)
            memcpy(static_cast<unsigned char*>(locked.pBits) + y * locked.Pitch, bytes + y * pitch, pitch);
        require(upload->UnlockRect());
        require(device->CreateQuery(D3DQUERYTYPE_EVENT, &completion));
        pending = true;
        const HRESULT updated = device->UpdateSurface(upload.Get(), nullptr, target.Get(), nullptr);
        const HRESULT issued = completion->Issue(D3DISSUE_END);
        fenced = SUCCEEDED(issued);
        require(issued);
        require(updated);
        HRESULT result;
        const ULONGLONG deadline = GetTickCount64() + 6000;
        while ((result = completion->GetData(nullptr, 0, D3DGETDATA_FLUSH)) == S_FALSE) {
            if (GetTickCount64() >= deadline) throw HRESULT_FROM_WIN32(WAIT_TIMEOUT);
            Sleep(1);
        }
        require(result);
        pending = false;
        require(device->CreateOffscreenPlainSurface(width, height, static_cast<D3DFORMAT>(format),
            D3DPOOL_SYSTEMMEM, &readback, nullptr));
        pending = true;
        fenced = false;
        require(device->GetRenderTargetData(target.Get(), readback.Get()));
        require(readback->LockRect(&locked, nullptr, D3DLOCK_READONLY));
        if (locked.Pitch < static_cast<INT>(pitch)) { readback->UnlockRect(); throw E_FAIL; }
        bool exact = true;
        for (UINT y = 0; y < height; ++y) {
            const auto row = static_cast<unsigned char*>(locked.pBits) + y * locked.Pitch;
            if (format == D3DFMT_X8R8G8B8) {
                // X is undefined storage, not authored alpha. RGB bits must remain exact.
                for (UINT x = 0; x < width; ++x) exact &= memcmp(row + x * 4, bytes + y * pitch + x * 4, 3) == 0;
            } else exact &= memcmp(row, bytes + y * pitch, pitch) == 0;
        }
        require(readback->UnlockRect());
        // LockRect has synchronized the readback; every defined source byte was inspected.
        pending = false;
        if (!exact) throw HRESULT_FROM_WIN32(ERROR_INVALID_DATA);
        if (names.FindSenderName(requested)) throw HRESULT_FROM_WIN32(ERROR_ALREADY_EXISTS);
        strcpy_s(name, requested);
        if (!access.ConfigureFrameCountLocal(false, false) || !access.CreateAccessMutex(name)) throw E_FAIL;
        if (!names.CreateSender(name, width, height, shared, registered_format)) throw E_FAIL;
        registered = true;
        if (strcmp(name, requested)) throw HRESULT_FROM_WIN32(ERROR_ALREADY_EXISTS);
    }
};
} // namespace

extern "C" spout_dx9_fixture_t* spout_dx9_fixture_create(unsigned int low, int high, const char* name,
    unsigned int format, unsigned int width, unsigned int height,
    const unsigned char* bytes, size_t length, unsigned int pitch, int* error) {
    if (!error) return nullptr;
    *error = E_FAIL;
    try {
        auto source = std::make_unique<Source>();
        source->create(low, high, name, format, width, height, bytes, length, pitch);
        *error = S_OK;
        return reinterpret_cast<spout_dx9_fixture_t*>(source.release());
    } catch (HRESULT result) { *error = result; return nullptr; }
      catch (...) { return nullptr; }
}

extern "C" void spout_dx9_fixture_destroy(spout_dx9_fixture_t* source) {
    try { delete reinterpret_cast<Source*>(source); } catch (...) {}
}

extern "C" void* spout_dx9_fixture_share_handle(spout_dx9_fixture_t* source) {
    try { return source ? reinterpret_cast<Source*>(source)->shared : nullptr; }
    catch (...) { return nullptr; }
}
