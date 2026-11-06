// Native adapter for .NET hosts, including engines with native plug-in support.
// Build gamectx-native for the target platform and make its library discoverable.
using System;
using System.Runtime.InteropServices;
using System.Text;

public sealed class GamectxClient : IDisposable
{
    [DllImport("gamectx_native", CallingConvention = CallingConvention.Cdecl)]
    static extern IntPtr gamectx_project_new(byte[] data, UIntPtr len);
    [DllImport("gamectx_native", CallingConvention = CallingConvention.Cdecl)]
    static extern IntPtr gamectx_project_request(IntPtr project, byte[] data, UIntPtr len);
    [DllImport("gamectx_native", CallingConvention = CallingConvention.Cdecl)]
    static extern IntPtr gamectx_last_error();
    [DllImport("gamectx_native", CallingConvention = CallingConvention.Cdecl)]
    static extern void gamectx_string_free(IntPtr value);
    [DllImport("gamectx_native", CallingConvention = CallingConvention.Cdecl)]
    static extern void gamectx_project_free(IntPtr project);
    readonly object gate = new object();
    IntPtr handle;
    static string Take(IntPtr value)
    {
        if (value == IntPtr.Zero) throw new InvalidOperationException("Native gamectx failed");
        try
        {
            int size = 0;
            while (Marshal.ReadByte(value, size) != 0) size++;
            byte[] bytes = new byte[size];
            Marshal.Copy(value, bytes, 0, size);
            return Encoding.UTF8.GetString(bytes);
        }
        finally { gamectx_string_free(value); }
    }
    public GamectxClient(string projectJson)
    {
        byte[] data = Encoding.UTF8.GetBytes(projectJson);
        handle = gamectx_project_new(data, (UIntPtr)data.Length);
        if (handle == IntPtr.Zero) throw new ArgumentException(Take(gamectx_last_error()));
    }
    // The host's JSON library builds {version:1,id,op,args} and decodes the reply.
    // Keeping JSON serialization outside this adapter avoids an engine-specific dependency.
    public string Request(string requestJson)
    {
        lock (gate)
        {
            if (handle == IntPtr.Zero) throw new ObjectDisposedException(nameof(GamectxClient));
            byte[] data = Encoding.UTF8.GetBytes(requestJson);
            IntPtr result = gamectx_project_request(handle, data, (UIntPtr)data.Length);
            if (result == IntPtr.Zero) throw new ArgumentException(Take(gamectx_last_error()));
            return Take(result);
        }
    }
    public void Dispose()
    {
        lock (gate)
        {
            if (handle != IntPtr.Zero) gamectx_project_free(handle);
            handle = IntPtr.Zero;
        }
        GC.SuppressFinalize(this);
    }
    ~GamectxClient() { Dispose(); }
}
