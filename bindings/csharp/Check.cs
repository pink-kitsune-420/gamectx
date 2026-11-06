// Build alongside GamectxClient.cs; pass examples/kit/project.json as the first argument.
using System;
using System.IO;
public static class Check
{
    public static int Main(string[] args)
    {
        using (var client = new GamectxClient(File.ReadAllText(args[0])))
        {
            var good = client.Request("{\"version\":1,\"id\":\"cs\",\"op\":\"doctor\",\"args\":{}}");
            if (!good.Contains("\"ok\":true") || !good.Contains("\"id\":\"cs\"")) return 1;
            var bad = client.Request("{\"version\":1,\"id\":\"cs\",\"op\":\"validate\",\"args\":{\"name\":\"inspect_item\",\"arguments\":{\"id\":42}}}");
            if (!bad.Contains("\"ok\":false")) return 1;
            if (args.Length == 3)
            {
                var requests = File.ReadAllLines(args[1]);
                var expected = File.ReadAllLines(args[2]);
                if (requests.Length != expected.Length) return 1;
                for (int i = 0; i < requests.Length; i++)
                {
                    var reply = client.Request(requests[i]);
                    if (expected[i] == "ERROR" ? !reply.Contains("\"ok\":false") : reply != expected[i]) return 1;
                }
                Console.WriteLine("C# fixture replies: " + requests.Length);
            }
        }
        Console.WriteLine("C# native checks passed");
        return 0;
    }
}
