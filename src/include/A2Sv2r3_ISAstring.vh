/*==============================================================================================================================

   Copyright © 2021 Advanced Architectures

   All rights reserved
   Confidential Information
   Limited Distribution to Authorized Persons Only
   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

   Project Name         : A2S v2r3
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

//`ifndef  A2Sv2r3_ISAstring
//`define  A2Sv2r3_ISAstring

function [31:0]   ISAstring;
input    [ 3:0]   FSM;
input    [ 5:0]   order;
input    [15:0]   fn;
input             YQmt;
input             mode;

localparam [4:0] START = 0, VECTOR= 1, FETCH= 2, NEXT = 4;
case({1'b1,mode})
   {FSM[START], 1'b1} : ISAstring = "STRT";
   {FSM[VECTOR],1'b1} : ISAstring = "VCTR";
   {FSM[FETCH], 1'b1} : ISAstring = "FTCH";
   default case(order)  // FSM[NEXT]
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
      res1  : ISAstring = "-1- ";   //________
      DECA  : ISAstring = "a-  ";
      DECB  : ISAstring = "b-  ";
      DECC  : ISAstring = "c-  ";
      SNB   : ISAstring = "snb ";
      res2  : ISAstring = "-2- ";
      res3  : ISAstring = "-3- ";
      res4  : ISAstring = "-4- ";
      NOT   : ISAstring = "not ";
      T2A   : ISAstring = ">a  ";
      T2B   : ISAstring = ">b  ";
      T2C   : ISAstring = ">c  ";
      T2R   : ISAstring = ">r  ";
      A2T   : ISAstring = "a>  ";
      B2T   : ISAstring = "b>  ";
      C2T   : ISAstring = "c>  ";
      R2T   : ISAstring = "r>  ";   //________
      NIP   : ISAstring = "nip ";
      DROP  : ISAstring = "drop";
      DUP   : ISAstring = "dup ";
      OVER  : ISAstring = "over";
      SWAP  : ISAstring = "><  ";
      ROT3  : ISAstring = "><< ";
      ROT4  : ISAstring = "><<<";
      SEL   : ISAstring = "?:  ";
      ADD   : ISAstring = "add ";
      SUB   : ISAstring = "sub ";
      SLT   : ISAstring = "<   ";
      ULT   : ISAstring = "u<  ";
      EQ    : ISAstring = "=   ";
      XOR   : ISAstring = "xor ";
      IOR   : ISAstring = "ior ";
      AND   : ISAstring = "and ";
      ZERO  : ISAstring = "zero";   //________
      ONE   : ISAstring = "one ";
      CO    : ISAstring = "co  ";
      RTN   : ISAstring = "rtn ";
      NOP   : ISAstring = "nop ";
      PFX   : ISAstring = "pfx ";
      LIT   : ISAstring = "lit ";
      EXT   : if (!YQmt) casez (fn[15:0])
                 IORC   : ISAstring = "cIO ";
                 IORD   : ISAstring = "rIO ";
                 IOWR   : ISAstring = "wIO ";
                 IOSW   : ISAstring = "sIO ";
                 RORI   : ISAstring = "<>>i";
                 ROLI   : ISAstring = "<<>i";
                 LSRI   : ISAstring = ">>i ";
                 LSLI   : ISAstring = "<<i ";
                 ASRI   : ISAstring = ">>>i";
                 CHARI  : ISAstring = "chai";
                 ROR    : ISAstring = "<>> ";
                 ROL    : ISAstring = "<<> ";
                 LSR    : ISAstring = ">>  ";
                 LSL    : ISAstring = "<<  ";
                 ASR    : ISAstring = ">>> ";
                 CHAR   : ISAstring = "char";
                 CC     : ISAstring = "cc  ";
                 SPILR  : ISAstring = "splr";
                 SPILD  : ISAstring = "spld";
                 FILLR  : ISAstring = "filr";
                 FILLD  : ISAstring = "fild";
                 POPC   : ISAstring = "popc";
                 EXCS   : ISAstring = "excs";
                 CLZ    : ISAstring = "clz ";
                 CTZ    : ISAstring = "ctz ";
                 BSWP   : ISAstring = "><b ";
                 MPY    : ISAstring = "*";
                 MPH    : ISAstring = "*h";
                 MPL    : ISAstring = "*l";
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
                 v128_load    :  ISAstring = "vlod";
                 v128_store   :  ISAstring = "vstr";
                 default: ISAstring = "fnXX";
                 endcase
              else        ISAstring = "fnMT";
      res5  : ISAstring = "-5- ";
      CALL  : ISAstring = "call";
      JN    : ISAstring = "n-> ";
      JZ    : ISAstring = "0-> ";
      JANZ  : ISAstring = "a-->";
      JBNZ  : ISAstring = "b-->";
      JCNZ  : ISAstring = "c-->";
      JMP   : ISAstring = "->  ";
      endcase
   endcase
endfunction

//`endif   //  A2Sv2r3_ISAstring

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
