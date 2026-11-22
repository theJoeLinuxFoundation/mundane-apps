"""Original harbour geometry encoded as MVT, shared by all fixture zoom levels."""
import math

def varint(n):
    b = bytearray()
    while n > 127:
        b.append((n & 127) | 128)
        n >>= 7
    b.append(n)
    return bytes(b)

def field(tag, payload):
    if isinstance(payload, int):
        return varint(tag << 3) + varint(payload)
    if isinstance(payload, str):
        payload = payload.encode()
    return varint(tag << 3 | 2) + varint(len(payload)) + payload

def harbour(z, tx, ty):
    # Feature coordinates below are logical pixels relative to the search result at zoom 12.
    n = 2**z
    x0 = (10.7522 + 180) / 360 * n
    y0 = (1 - math.asinh(math.tan(math.radians(59.9139))) / math.pi) / 2 * n
    factor = 2**(z-12) * 16
    def coords(p):
        return (round((x0-tx)*4096 + p[0]*factor), round((y0-ty)*4096 + p[1]*factor))
    def layer(name, features):
        values = []
        out = field(15, 2) + field(1, name) + field(5, 4096)
        for kind, prop, shape in features:
            if prop not in values:
                values.append(prop)
            pts = list(map(coords, shape))
            packed = []
            old = (0, 0)
            for i, p in enumerate(pts):
                if i == 0:
                    packed.append(9)
                if i == 1:
                    packed.append((len(pts)-1)*8 + 2)
                for a,b in zip(p,old):
                    d = a-b
                    packed.append(d*2 if d>=0 else -d*2-1)
                old = p
            if kind == 3:
                packed.append(15)
            geometry = b"".join(map(varint, packed))
            out += field(2, field(2, varint(0)+varint(values.index(prop))) + field(3,kind) + field(4,geometry))
        out += field(3, "name" if name == "place" else "class")
        return field(3, out + b"".join(field(4, field(1,v)) for v in values))
    shore = [(-50,-650),(-20,-250),(65,-140),(82,-65),(14,-32),(35,10),
             (107,36),(135,80),(80,140),(132,260),(308,620)]
    road = [(-250,-600),(-165,-200),(-110,-80),(-35,-35),(-25,35),(46,100),(60,320)]
    streets = [(2,"primary",road)]
    for y in range(-180,181,40):
        streets.append((2,"residential",[(-270,y-10),(-155,y+5),(-108,y),(-60,y+7)]))
    for x in range(-240,-65,38):
        streets.append((2,"residential",[(x,-185),(x+8,-65),(x-9,40),(x+7,186)]))
    buildings = [(3,"building",[(x,y),(x+18,y-1),(x+19,y+11),(x,y+12)])
                 for x in range(-225,-40,38) for y in range(-154,151,40)]
    return b"".join([
        layer("landuse",[(3,"residential",[(-310,-210),(-80,-220),(-32,-52),(50,195),(-285,213)])]),
        layer("park",[(3,"park",[(-160,-160),(-90,-153),(-56,-82),(-121,-68)]),
                      (3,"park",[(-190,96),(-141,91),(-124,166),(-190,170)])]),
        layer("water",[(3,"bay",shore+[(800,620),(800,-650)])]),
        layer("building",buildings),layer("transportation",streets),
        layer("place",[(1,"North Quay",[(-64,-50)]),(1,"Survey Park",[(-142,-118)]),(1,"East Bay",[(108,15)])])])
