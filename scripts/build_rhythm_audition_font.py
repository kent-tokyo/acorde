#!/usr/bin/env python3
"""Build original synthetic audition samples; no downloaded or recorded material."""
import math, struct, sys
from pathlib import Path
RATE=22050

def chunk(tag, payload):
    if tag in (b'isng', b'INAM', b'ICOP') and len(payload) % 2:
        payload += b'\0'
    return tag+struct.pack('<I',len(payload))+payload+(b'\0' if len(payload)%2 else b'')
def name(s): return s.encode('ascii')[:19].ljust(20,b'\0')
def generator(op,value): return struct.pack('<HH',op,value & 65535)
def wave(kind,root,seconds):
    frequency=440*2**((root-69)/12)
    noise=123456789
    result=[]
    for i in range(int(RATE*seconds)):
        t=i/RATE
        noise=(1664525*noise+1013904223) & 0xffffffff
        n=noise/2147483648-1
        if kind=='kick':
            phase=2*math.pi*(48*t+85*.035*(1-math.exp(-t/.035)))
            value=.70*math.sin(phase)*math.exp(-t/0.11)+.12*n*math.exp(-t/.006)
        elif kind=='muted':
            value=(.45*math.sin(2*math.pi*frequency*t)+.14*n*math.exp(-t/.025))*math.exp(-t/.07)
        else:
            harmonics=[1,.35,.13] if kind=='bass' else [1,.3,.17,.07]
            value=sum(a*math.sin(2*math.pi*frequency*t*(j+1)) for j,a in enumerate(harmonics))
            value*=.35*math.exp(-t/(1.8 if kind=='bass' else 1.0))
        value*=min(1,t/.002)*min(1,(seconds-t)/.01)
        result.append(round(max(-1,min(1,value))*32767))
    return result

def build():
    definitions=[('Soft keys',0,0,'keys',60,3.0),('Round bass',0,33,'bass',40,3.0),('Muted bass',0,34,'muted',40,.5),('Kick kit',128,0,'kick',36,.7)]
    pcm=[];headers=[];phdr=b'';inst=b'';igen=b'';ibag=b''
    for i,(label,bank,program,kind,root,secs) in enumerate(definitions):
        values=wave(kind,root,secs);start=len(pcm);pcm+=values;end=len(pcm);pcm += [0]*46
        headers.append(struct.pack('<20sIIIIIBbHH',name(label),start,end,start,end,RATE,root,0,0,1))
        phdr+=struct.pack('<20sHHHIII',name(label),program,bank,i,0,0,0)
        inst+=struct.pack('<20sH',name(label),i)
        ibag+=struct.pack('<HH',len(igen)//4,0)
        for op,value in [(43,36|(36<<8))] if kind=='kick' else [(43,127<<8)]: igen+=generator(op,value)
        for op,value in [(34,-12000),(38,-6000),(54,0),(56,0 if kind=='kick' else 100),(58,root),(53,i)]: igen+=generator(op,value)
    count=len(definitions)
    phdr+=struct.pack('<20sHHHIII',name('EOP'),0,0,count,0,0,0)
    inst+=struct.pack('<20sH',name('EOI'),count)
    ibag+=struct.pack('<HH',len(igen)//4,0);igen+=generator(0,0)
    headers.append(struct.pack('<20sIIIIIBbHH',name('EOS'),len(pcm),len(pcm),len(pcm),len(pcm),RATE,0,0,0,1))
    info=chunk(b'LIST',b'INFO'+chunk(b'ifil',struct.pack('<HH',2,1))+chunk(b'isng',b'EMU8000\0')+chunk(b'INAM',b'Acorde rhythm audition\0')+chunk(b'ICOP',b'Original synthesized samples; same license as acorde repository\0'))
    sdta=chunk(b'LIST',b'sdta'+chunk(b'smpl',struct.pack('<'+'h'*len(pcm),*pcm)))
    pdta=chunk(b'LIST',b'pdta'+b''.join([chunk(b'phdr',phdr),chunk(b'pbag',b''.join(struct.pack('<HH',i,0) for i in range(count+1))),chunk(b'pmod',b'\0'*10),chunk(b'pgen',b''.join(generator(41,i) for i in range(count))+generator(0,0)),chunk(b'inst',inst),chunk(b'ibag',ibag),chunk(b'imod',b'\0'*10),chunk(b'igen',igen),chunk(b'shdr',b''.join(headers))]))
    return chunk(b'RIFF',b'sfbk'+info+sdta+pdta)
if __name__=='__main__':
    Path(sys.argv[1]).write_bytes(build())
