' roByteArray conversions.
sub Main()
    print "@@PROBE_BEGIN"
    bytes = CreateObject("roByteArray")
    bytes.FromAsciiString("Hello")
    print type(bytes), bytes.Count(), bytes[0], bytes.ToHexString(), bytes.ToBase64String()
    bytes.FromHexString("00ff7f80")
    print bytes.Count(), bytes[1], bytes[3], bytes.GetSignedByte(1), bytes.ToHexString()
    bytes.FromBase64String("Um9rdQ==")
    print bytes.ToAsciiString(), bytes.Count()
    bytes.Push(33)
    print bytes.ToAsciiString(), bytes.GetCRC32()
    other = CreateObject("roByteArray")
    other.Push(300)
    other.Push(-1)
    print other[0], other[1], other.ToHexString()
    print "@@PROBE_END"
end sub
