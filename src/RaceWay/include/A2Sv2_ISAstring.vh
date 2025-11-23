/*==============================================================================================================================

   Copyright © 2021 Advanced Architectures

   All rights reserved
   Confidential Information
   Limited Distribution to Authorized Persons Only
   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

   Project Name         : A2S v2
   Description          : ISA order encoding

================================================================================================================================
   THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
================================================================================================================================
   Notes for Use and Synthesis
==============================================================================================================================*/

function [31:0]   ISAstring;
input    [ 4:0]   FSM;
input    [ 5:0]   order;
input    [15:0]   fn;
input             YQmt;
input             mode;

localparam [4:0] START = 0, VECTOR = 1, FETCH = 2, NEXT = 3, OOPS = 4, ARGH = 5;
case({1'b1,mode})
   {FSM[START], 1'b1} : ISAstring = "STRT";
   {FSM[VECTOR],1'b1} : ISAstring = "VCTR";
   {FSM[OOPS ], 1'b1} : ISAstring = "OOPS";
   {FSM[ARGH ], 1'b1} : ISAstring = "ARGH";
   {FSM[FETCH], 1'b1} : ISAstring = "FTCH";
   default case(order)
      PUTA  : ISAstring = "!a  ";
      PUTB  : ISAstring = "!b  ";
      PUTC  : ISAstring = "!c  ";
      PUT   : ISAstring = "!   ";
      GETA  : ISAstring = "@a  ";
      GETB  : ISAstring = "@b  ";
      GETC  : ISAstring = "@c  ";
      GET   : ISAstring = "@   ";
      PTAP  : ISAstring = "!a+ ";
      PTBP  : ISAstring = "!b+ ";
      PTCP  : ISAstring = "!c+ ";
      XCHG  : ISAstring = "@!  ";
      GTAP  : ISAstring = "@a+ ";
      GTBP  : ISAstring = "@b+ ";
      GTCP  : ISAstring = "@c+ ";
      res1  : ISAstring = "-1--";
      PTMA  : ISAstring = "!-a ";
      PTMB  : ISAstring = "!-b ";
      PTMC  : ISAstring = "!-c ";
      res2  : ISAstring = "-2--";
      A2T   : ISAstring = "a>  ";
      B2T   : ISAstring = "b>  ";
      C2T   : ISAstring = "c>  ";
      R2T   : ISAstring = "r>  ";
      T2A   : ISAstring = ">a  ";
      T2B   : ISAstring = ">b  ";
      T2C   : ISAstring = ">c  ";
      T2R   : ISAstring = ">r  ";
      AZ    : ISAstring = "a0= ";
      BZ    : ISAstring = "b0= ";
      CZ    : ISAstring = "c0= ";
      DUP   : ISAstring = "dup ";
      AND   : ISAstring = "and ";
      XOR   : ISAstring = "xor ";
      IOR   : ISAstring = "ior ";
      res3  : ISAstring = "-3--";
      OVER  : ISAstring = "over";
      SWAP  : ISAstring = "><  ";
      RT3   : ISAstring = "><< ";
      RT4   : ISAstring = "><<<";
      ADD   : ISAstring = "add ";
      SUB   : ISAstring = "sub ";
      NIP   : ISAstring = "nip ";
      DROP  : ISAstring = "drop";
      ZLT   : ISAstring = "0<  ";
      ZEQ   : ISAstring = "0=  ";
      ZGT   : ISAstring = "0>  ";
      NOT   : ISAstring = "not ";
      RTN   : ISAstring = "rtn ";
      CO    : ISAstring = "co  ";
      ZERO  : ISAstring = "zero";
      ONE   : ISAstring = "one ";
      res4  : ISAstring = "-4--";
      PFX   : ISAstring = "pfx ";
      FN    : if (!YQmt) casez (fn[15:0])
                 IORC   : ISAstring = "cIO ";
                 IORD   : ISAstring = "rIO ";
                 IOWR   : ISAstring = "wIO ";
                 IOSW   : ISAstring = "sIO ";
                 LSLI   : ISAstring = "<<i ";
                 LSRI   : ISAstring = ">>i ";
                 ASLI   : ISAstring = "a>>i";
                 ASRI   : ISAstring = "a<<i";
                 LSLN   : ISAstring = "<<  ";
                 LSRN   : ISAstring = ">>  ";
                 ASLN   : ISAstring = "a<< ";
                 ASRN   : ISAstring = "a>> ";
                 CC     : ISAstring = "cc  ";
                 POPC   : ISAstring = "popc";
                 SWBYT  : ISAstring = "bswp";
                 UNSL   : ISAstring = "ult ";
                 FMAD   : ISAstring = "fmad";
                 FMSB   : ISAstring = "fmsb";
                 FMNA   : ISAstring = "fmna";
                 FMNS   : ISAstring = "fmns";
                 FMUL   : ISAstring = "fmul";
                 FADD   : ISAstring = "fadd";
                 FSUB   : ISAstring = "fsub";
                 FRSB   : ISAstring = "frsb";
                 FDIV   : ISAstring = "fdiv";
                 FREM   : ISAstring = "frem";
                 FMOD   : ISAstring = "fmod";
                 FSQR   : ISAstring = "fsqr";
                 FS2F   : ISAstring = "fs2f";
                 FF2S   : ISAstring = "ff2s";
                 FU2F   : ISAstring = "fu2f";
                 FF2U   : ISAstring = "ff2u";
                 FCLT   : ISAstring = "fclt";
                 FCEQ   : ISAstring = "fceq";
                 FCLE   : ISAstring = "fcle";
                 FCGT   : ISAstring = "fcgt";
                 FCNE   : ISAstring = "fcne";
                 FCGE   : ISAstring = "fcge";
                 FMAX   : ISAstring = "fmax";
                 FMIN   : ISAstring = "fmin";
                 FSAT   : ISAstring = "fsat";
                 FABS   : ISAstring = "fabs";
                 FNAN   : ISAstring = "fnan";
                 FCHS   : ISAstring = "fchs";
                 default: ISAstring = "fnXX";
                 endcase
              else        ISAstring = "fnMT";
      CALL  : ISAstring = "call";
      LIT   : ISAstring = "lit ";
      JMP   : ISAstring = "->  ";
      JZ    : ISAstring = "0-> ";
      JN    : ISAstring = "n-> ";
      JDANZ : ISAstring = "a-->";
      JDBNZ : ISAstring = "b-->";
      JDCNZ : ISAstring = "c-->";
      NOP   : ISAstring = "nop ";
      endcase
   endcase
endfunction

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
