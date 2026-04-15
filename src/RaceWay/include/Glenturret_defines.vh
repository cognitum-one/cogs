/*==============================================================================================================================

   Copyright © 2003 Advanced Architectures

   All rights reserved
   Confidential Information
   Limited Distribution to Authorized Persons Only
   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

   Project Name         : Edradour
   Description          : Configuration file

   Author               : RTT
   Version              : 1.0
   Creation Date        : 5/27/2021

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

`ifndef  GLENTURRET_DEFINES
`define  GLENTURRET_DEFINES

`timescale  1ns/1ps

`define  TILE_CLOCK_PERIOD    0.240                               // ns
`define  FAST_CLOCK_PERIOD    0.080                               // ns

`define  fastcycle            #(`FAST_CLOCK_PERIOD * 0.500)       // Half period  =>  80ps cycle => 12.5  GHz
`define  slowcycle            #(`TILE_CLOCK_PERIOD * 0.500)       // Half period  => 240ps cycle =>  4.16 GHz
`define  min_delay            #0.05                               // Must be greater than half fast cycle time
`define  tFCQ                 #(`FAST_CLOCK_PERIOD * 0.100)       // ns
`define  tFGQ                 #(`FAST_CLOCK_PERIOD * 0.250)       // ns
`define  tCQ                  #(`TILE_CLOCK_PERIOD * 0.100)       // ns
`define  tACC                 #(`TILE_CLOCK_PERIOD * 0.500)       // ns
`define  accCM                0.240                               // ns
`define  accDM                0.240                               // ns
`define  accST                0.180                               // ns
`define  accWM                0.250                               // ns
`define  cycCM                0.348                               // ns
`define  cycDM                0.348                               // ns
`define  cycST                0.270                               // ns
`define  CM_NUM_CLOCKS_READ   1
`define  CM_NUM_CLOCKS_CYCLE  1
`define  DM_NUM_CLOCKS_READ   1
`define  DM_NUM_CLOCKS_CYCLE  1

`ifndef   A2_DEFINES
`include "A2_defines.vh"
`endif

//`ifndef   A2Sv2r2_DEFINES        //Joel
//`include "A2Sv2r2_defines.vh"    //Joel
//`endif                           //Joel
                                       //      97      96    95:88     87:80     79:72     71:64     63:32      31:0
`define  XMOSI_WIDTH 1+1+8+8+8+8+32+32 //  98={xresetn,xpush,xcmd[7:0],xtag[7:0],xdst[7:0],xsrc[7:0],xwrd[31:0],xadr[31:0]}
`define  XMISO_WIDTH 1+1+8+8+8+8+32+32 //  98={xtmo,   xordy,xack[7:0],xtag[7:0],xdst[7:0],xsrc[7:0],xrd0[31:0],xrd1[31:0]}

`define  BMOSI_WIDTH 8+8+8+8+32+32     //  96={              bcmd[7:0],btag[7:0],bdst[7:0],bsrc[7:0],bwrd[31:0],badr[31:0]}
`define  BMISO_WIDTH 8+8+8+8+32+32     //  96={              back[7:0],btag[7:0],bdst[7:0],bsrc[7:0],brd0[31:0],brd1[31:0]}

`define  SMOSI_WIDTH 1+4+1+10+256      // 272={sreq,sen[3:0],swr,sadr[9:0],swrd[255:0]}
`define  SMISO_WIDTH 4+256             // 260={sack,sbrd}

`define  NMOSI_WIDTH 1+4+1+10+256      // 272={nreq,nen[3:0],nwr,nadr[9:0],nwrd[255:0]}
`define  NMISO_WIDTH 4+256             // 260={nack,nrdd[255:0]}

`define  TMOSI_WIDTH 1+1+32+32         //  66={treq,twr,  twrd[31:0],tadr[31:0]}
`define  TMISO_WIDTH 1+1+32+32         //  66={tgnt,tack, trd0[31:0],trd1[31:0]}

`define  XPUSH 96
`define  XORDY 96
`define  XREQ  95
`define  XNACK 95
`define  XERR  94

// Raceway command codes
`define  SYSWR    8'b10010000       // System Unicast Write (to clock register)
`define  UNIWR    8'b10010001       // Unicast Write
`define  UNIWS    8'b1001?001       // Unicast Write or Swap
`define  UNIRD    8'b10001001       // Unicast Read
`define  UNIER    8'b00000000       // Unicast Error Acknowledge
`define  UNIWK    8'b00001001       // Unicast Write Acknowledge
`define  UNISK    8'b0001?001       // Unicast Write or Swap Acknowledge
`define  UNIRK    8'b00001010       // Unicast Read  Acknowledge
`define  BCTWR    8'b10110001       // Broadcast Write RAM
`define  BCTRL    8'b10100000       // Broadcast Release Reset
`define  BCTRS    8'b10111000       // Broadcast Set     Reset
// Raceway TILE addresses
`define  TILEZ    8'b00000000

// Register Addressing ===========================================================================================

// Tile -- IMOSI Address
`define  aSHA256_base       13'h1000
`define  aSHA256_MESG       13'h1000
`define  aSHA256_HASH       13'h1001
`define  aSHA256_STOP       13'h1002
`define  aSHA256_STRT       13'h1003
`define  aSHA256_INIT       13'h1004
`define  aSHA256_WAKE       13'h1006
`define  aSHA256_SLEEP      13'h1007

`define  aXSALSA_base       13'h1008
`define  aXSALSA_XBUF       13'h1008
`define  aXSALSA_XBBS       13'h1009
`define  aXSALSA_STOP       13'h100a
`define  aXSALSA_STRT       13'h100b

`define  aGATEWY_base       13'h1010
`define  aGATEWY_MyID       13'h1010
`define  aGATEWY_ADDR       13'h1011
`define  aGATEWY_DATA       13'h1012
`define  aGATEWY_CMND       13'h1013
`define  aGATEWY_BUSY       13'h1014
`define  aGATEWY_RDA1       13'h1015
`define  aGATEWY_RDA0       13'h1016
`define  aGATEWY_RESP       13'h1017
`define  aGATEWY_TIMO       13'h1018
`define  aGATEWY_TIMB       13'h1019
`define  aGATEWY_TIMR       13'h101a
`define  aGATEWY_FLAG       13'h101b

`define  aCLOCKS_base       13'h1020
`define  aCLOCKS_READ       13'h1020

`define  aMATCH_TAGS        13'h1030
`define  aSCRATCH           13'h1031

`define  aBMVM_base         13'h1040
`define  aBMVM_THLD         13'h1040
`define  aBMVM_ITER         13'h1041
`define  aBMVM_DIMN         13'h1042
`define  aBMVM_ADR0         13'h1044
`define  aBMVM_ADR1         13'h1045
`define  aBMVM_RESV         13'h1047
`define  aBMVM_VEC0         13'h1048
`define  aBMVM_VEC1         13'h1049
`define  aBMVM_VEC2         13'h104a
`define  aBMVM_VEC3         13'h104b
`define  aBMVM_CTRL         13'h104f

// xmosi\bmosi addresses -- DERIVED from imosi addresses ---( don't touch ) -------------------------------------------------------------

`define  aTILE_SHA256_MESG  {16'h_c000,1'b0,`aSHA256_MESG,2'b00}
`define  aTILE_SHA256_HASH  {16'h_c000,1'b0,`aSHA256_HASH,2'b00}
`define  aTILE_SHA256_STOP  {16'h_c000,1'b0,`aSHA256_STOP,2'b00}
`define  aTILE_SHA256_STRT  {16'h_c000,1'b0,`aSHA256_STRT,2'b00}
`define  aTILE_SHA256_INIT  {16'h_c000,1'b0,`aSHA256_INIT,2'b00}
`define  aTILE_SHA256_WAKE  {16'h_c000,1'b0,`aSHA256_WAKE,2'b00}
`define  aTILE_SHA256_SLEEP {16'h_c000,1'b0,`aSHA256_SLEEP,2'b00}

`define  aTILE_XSALSA_XBUF  {16'h_c000,1'b0,`aXSALSA_XBUF,2'b00}
`define  aTILE_XSALSA_XBBS  {16'h_c000,1'b0,`aXSALSA_XBBS,2'b00} // Byte Swap
`define  aTILE_XSALSA_STOP  {16'h_c000,1'b0,`aXSALSA_STOP,2'b00}
`define  aTILE_XSALSA_STRT  {16'h_c000,1'b0,`aXSALSA_STRT,2'b00}

`define  aTILE_GATEWY_ADDR  {16'h_c000,1'b0,`aGATEWY_ADDR,2'b00}
`define  aTILE_GATEWY_DATA  {16'h_c000,1'b0,`aGATEWY_DATA,2'b00}
`define  aTILE_GATEWY_CMND  {16'h_c000,1'b0,`aGATEWY_CMND,2'b00}
`define  aTILE_GATEWY_TIMO  {16'h_c000,1'b0,`aGATEWY_TIMO,2'b00}
`define  aTILE_GATEWY_RDA1  {16'h_c000,1'b0,`aGATEWY_RDA1,2'b00}
`define  aTILE_GATEWY_RDA0  {16'h_c000,1'b0,`aGATEWY_RDA0,2'b00}
`define  aTILE_GATEWY_RESP  {16'h_c000,1'b0,`aGATEWY_RESP,2'b00}
`define  aTILE_GATEWY_BTGS  {16'h_c000,1'b0,`aGATEWY_BTGS,2'b00}
`define  aTILE_GATEWY_BUSY  {16'h_c000,1'b0,`aGATEWY_BUSY,2'b00}

`define  aTILE_CLOCKS_READ  {16'h_c000,1'b0,`aCLOCKS_READ,2'b00}

`define  aTILE_MATCH_TAGS   {16'h_c000,1'b0,`MATCH_TAGS,  2'b00}
`define  aTILE_SCRATCH      {16'h_c000,1'b0,`SCRATCH,     2'b00}

`define  aTILE_BMVM_THLD    {16'h_c000,1'b0,`aBMVM_THLD,  2'b00}
`define  aTILE_BMVM_ITER    {16'h_c000,1'b0,`aBMVM_ITER,  2'b00}
`define  aTILE_BMVM_DIMN    {16'h_c000,1'b0,`aBMVM_DIMN,  2'b00}
`define  aTILE_BMVM_CTRL    {16'h_c000,1'b0,`aBMVM_CTRL,  2'b00}
`define  aTILE_BMVM_ADR0    {16'h_c000,1'b0,`aBMVM_ADR0,  2'b00}
`define  aTILE_BMVM_ADR1    {16'h_c000,1'b0,`aBMVM_ADR1,  2'b00}
`define  aTILE_BMVM_RESV    {16'h_c000,1'b0,`aBMVM_RESV,  2'b00}
`define  aTILE_BMVM_VEC0    {16'h_c000,1'b0,`aBMVM_VEC0,  2'b00}
`define  aTILE_BMVM_VEC1    {16'h_c000,1'b0,`aBMVM_VEC1,  2'b00}
`define  aTILE_BMVM_VEC2    {16'h_c000,1'b0,`aBMVM_VEC2,  2'b00}
`define  aTILE_BMVM_VEC3    {16'h_c000,1'b0,`aBMVM_VEC3,  2'b00}

`define  aTILE_SET_CLOCKS    32'hffff_0000
`define  aTILE_RESET         32'hffff_ffff
`define  aTILE_RELEASE       32'hffff_8000

// Local DMA NEWS
`define  LMOSI_WIDTH       66
`define  LMOSI_EN          65
`define  LMOSI_WR          64
`define  LMOSI_DW          63:32
`define  LMOSI_MS          31:30                   // Memory Select Address (CM,DM,WM,IO)
`define  LMOSI_AD          31:0
`define  LMISO_WIDTH  33
`define  LMISO_DR          31:0
`define  LMISO_AK          32
`endif

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
