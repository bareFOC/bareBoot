import time, json, urllib.request, sys, hashlib
BASE="http://10.10.10.10"
IMG_PATH=sys.argv[1] if len(sys.argv)>1 else "/tmp/opencode/n20-flash-test.bin"

def post(path,payload):
    req=urllib.request.Request(BASE+path,data=json.dumps(payload).encode(),headers={"Content-Type":"application/json"},method="POST")
    try:
        with urllib.request.urlopen(req,timeout=10) as r: return json.loads(r.read().decode())
    except Exception as e: return {"err":str(e)}

def crc16(data):
    c=0xffff
    for d in data:
        v=d^(c&0xff); v^=(v<<4)&0xff; c=(((v<<8)|(c>>8))^(v>>4)^(v<<3))&0xffff
    return c

def frame(cmd,payload=b''):
    assert len(payload)%4==0
    wc=len(payload)//4; core=bytes([cmd,wc])+payload; c=crc16(core)
    return bytes([0x01,0x88])+core+bytes([c&0xff,(c>>8)&0xff,0x99,0x03])

def parse(data):
    for i in range(len(data)-1):
        if data[i]==0x01 and data[i+1]==0x88 and len(data)-i>=8:
            cmd=data[i+2]; wc=data[i+3]; fl=4+wc*4+4
            if len(data)-i<fl: continue
            cand=data[i:i+fl]
            if cand[-2]!=0x99 or cand[-1]!=0x03: continue
            if crc16(cand[2:4+wc*4])!=(cand[-4]|(cand[-3]<<8)): continue
            return {"cmd":cand[2],"wc":wc,"payload":cand[4:4+wc*4]}
    return None

def xfer_write(hexstr):
    return post("/api/v1/bus/i2c/transfer",{"addr":0x40,"data_hex":hexstr,"read_len":0})
def xfer_read(n=32):
    return post("/api/v1/bus/i2c/transfer",{"addr":0x40,"data_hex":"","read_len":n})

def exchange(cmd,payload=b'',wait=0.03,rd=32):
    r=xfer_write(frame(cmd,payload).hex())
    if r.get("status")!="ok": return None,r
    time.sleep(wait)
    r=xfer_read(rd)
    p=parse(bytes.fromhex(r.get("rx_hex",""))) if r.get("rx_hex") else None
    return p,r

# Reset target via nRST
f=open("/dev/hidraw5","r+b",buffering=0)
def dap(b):
    p=bytes([0])+bytes(b); p+=bytes(65-len(p)); f.write(p[:65]); return f.read(64)
def rst(low): return dap([0x10,0x00 if low else 0x80,0x80,0,0,0,0])[:4].hex()
print("reset:",rst(True),rst(False)); f.close()

# NOTE: no raw 0xF0 strobe here. Entry is via hardware nRST within the
# boot window; a raw F0 byte is not a framed Katapult command and leaves
# the Katapult console's TX staging in an inconsistent state.
p,r=exchange(0x11,wait=0.05,rd=128)
if not p or p["cmd"]!=0xA0:
    print("CONNECT FAILED",p,r); sys.exit(1)
pl=p["payload"]
app_base=int.from_bytes(pl[8:12],"little"); bs=int.from_bytes(pl[12:16],"little")
mcu=pl[16:].split(b'\0')[0].decode(errors='replace')
print(f"CONNECT OK: app_base=0x{app_base:08x} block_size={bs} mcu={mcu}")

img=open(IMG_PATH,"rb").read()
print(f"image: {len(img)} bytes sha256={hashlib.sha256(img).hexdigest()[:16]}")
nb=(len(img)+bs-1)//bs
t0=time.time()
fails=0
for i in range(nb):
    blk=img[i*bs:(i+1)*bs]
    blk=blk+b'\xff'*(bs-len(blk))
    addr=app_base+i*bs
    payload=addr.to_bytes(4,"little")+blk
    # page flush every 128 blocks; allow extra time
    wait=0.30 if (i+1)%128==0 else 0.02
    p,r=exchange(0x12,payload,wait=wait,rd=24)
    if not p or p["cmd"]!=0xA0:
        print(f"BLOCK {i} FAIL addr=0x{addr:08x} p={p} r={r}"); fails+=1
        if fails>3: sys.exit(2)
    if (i+1)%200==0:
        print(f"  {i+1}/{nb} blocks, {time.time()-t0:.1f}s")
print(f"blocks done in {time.time()-t0:.1f}s, fails={fails}")
if fails: sys.exit(3)

# RX_EOF (flushes final partial page -> allow extra time)
p,r=exchange(0x13,wait=0.30,rd=24)
print("RX_EOF:",p and hex(p["cmd"]), p and p["payload"].hex())
if not p or p["cmd"]!=0xA0: sys.exit(4)

# Readback verification via REQ_BLOCK at several offsets
print("=== I2C readback verification ===")
checks=[0,1,nb//2,nb-2,nb-1]
ok=True
for bi in checks:
    addr=app_base+bi*bs
    p,r=exchange(0x14,addr.to_bytes(4,"little"),wait=0.05,rd=96)
    if not p or p["cmd"]!=0xA0 or len(p["payload"])<8+bs:
        print(f"REQ {bi} FAIL",p); ok=False; continue
    got=p["payload"][8:8+bs]
    exp=img[bi*bs:bi*bs+bs]
    exp=exp+b'\xff'*(bs-len(exp))
    match=got==exp
    print(f"REQ block {bi} @0x{addr:08x}: {'MATCH' if match else 'MISMATCH'}")
    if not match:
        print("  got:",got[:16].hex(),"exp:",exp[:16].hex()); ok=False
if not ok: sys.exit(5)

# COMPLETE -> jump to app
p,r=exchange(0x15,wait=0.05,rd=24)
print("COMPLETE:",p and hex(p["cmd"]))
print("FLASH+VERIFY SUCCESS")
