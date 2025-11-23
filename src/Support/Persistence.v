/*==============================================================================================================================

    Copyright © 2025 Advanced Architectures

    All rights reserved
    Confidential Information
    Limited Distribution to Authorized Persons Only
    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

    Project Name         : Persistence
                           Persistence check on an input signal

    Description          : Power-On Reset and start-up mechanism

================================================================================================================================
    THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
    BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
    TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
    AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
    ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
================================================================================================================================
    Notes for Use and Synthesis


===============================================================================================================================*/

//`define PureRTL
`timescale  1ps/10fs

module Persistence (
   output wire y,
   input  wire a
   );

wire  [15:0]   delay, A;

`ifdef   PureRTL
assign #130 delay[00]   =  ~a,
            delay[01]   =   delay[00],
            delay[02]   =   delay[01],
            delay[03]   =   delay[02],
            delay[04]   =   delay[03],
            delay[05]   =   delay[04],
            delay[06]   =   delay[05],
            delay[07]   =   delay[06],
            delay[08]   =   delay[07],
            delay[09]   =   delay[08],
            delay[10]   =   delay[09],
            delay[11]   =   delay[10],
            delay[12]   =   delay[11],
            delay[13]   =   delay[12],
            delay[14]   =   delay[13],
            delay[15]   =   delay[14];

`elsif REAL_DEVICE
wire tie_low  = 1'b0;
wire tie_high = 1'b1;
DLY10_X1N_A7P5PP84TR_C14 rst_del1 (.Y(delay[00]), .VDD(tie_high), .VNW(tie_high), .VPW(tie_low), .VSS(tie_low), .A(~a)); //JLPL - 11_9_25
DLY10_X1N_A7P5PP84TR_C14 rst_del2 (.Y(delay[01]), .VDD(tie_high), .VNW(tie_high), .VPW(tie_low), .VSS(tie_low), .A(delay[00]));
DLY10_X1N_A7P5PP84TR_C14 rst_del3 (.Y(delay[02]), .VDD(tie_high), .VNW(tie_high), .VPW(tie_low), .VSS(tie_low), .A(delay[01]));
DLY10_X1N_A7P5PP84TR_C14 rst_del4 (.Y(delay[03]), .VDD(tie_high), .VNW(tie_high), .VPW(tie_low), .VSS(tie_low), .A(delay[02]));
DLY10_X1N_A7P5PP84TR_C14 rst_del5 (.Y(delay[04]), .VDD(tie_high), .VNW(tie_high), .VPW(tie_low), .VSS(tie_low), .A(delay[03]));
DLY10_X1N_A7P5PP84TR_C14 rst_del6 (.Y(delay[05]), .VDD(tie_high), .VNW(tie_high), .VPW(tie_low), .VSS(tie_low), .A(delay[04]));
DLY10_X1N_A7P5PP84TR_C14 rst_del7 (.Y(delay[06]), .VDD(tie_high), .VNW(tie_high), .VPW(tie_low), .VSS(tie_low), .A(delay[05]));
DLY10_X1N_A7P5PP84TR_C14 rst_del8 (.Y(delay[07]), .VDD(tie_high), .VNW(tie_high), .VPW(tie_low), .VSS(tie_low), .A(delay[06]));
DLY10_X1N_A7P5PP84TR_C14 rst_del9 (.Y(delay[08]), .VDD(tie_high), .VNW(tie_high), .VPW(tie_low), .VSS(tie_low), .A(delay[07]));
DLY10_X1N_A7P5PP84TR_C14 rst_del10 (.Y(delay[09]), .VDD(tie_high), .VNW(tie_high), .VPW(tie_low), .VSS(tie_low), .A(delay[08]));
DLY10_X1N_A7P5PP84TR_C14 rst_del11 (.Y(delay[10]), .VDD(tie_high), .VNW(tie_high), .VPW(tie_low), .VSS(tie_low), .A(delay[09]));
DLY10_X1N_A7P5PP84TR_C14 rst_del12 (.Y(delay[11]), .VDD(tie_high), .VNW(tie_high), .VPW(tie_low), .VSS(tie_low), .A(delay[10]));
DLY10_X1N_A7P5PP84TR_C14 rst_del13 (.Y(delay[12]), .VDD(tie_high), .VNW(tie_high), .VPW(tie_low), .VSS(tie_low), .A(delay[11]));
DLY10_X1N_A7P5PP84TR_C14 rst_del14 (.Y(delay[13]), .VDD(tie_high), .VNW(tie_high), .VPW(tie_low), .VSS(tie_low), .A(delay[12]));
DLY10_X1N_A7P5PP84TR_C14 rst_del15 (.Y(delay[14]), .VDD(tie_high), .VNW(tie_high), .VPW(tie_low), .VSS(tie_low), .A(delay[13]));
DLY10_X1N_A7P5PP84TR_C14 rst_del16 (.Y(delay[15]), .VDD(tie_high), .VNW(tie_high), .VPW(tie_low), .VSS(tie_low), .A(delay[14]));
`else
DLY10_X1N_A7P5PP84TR_C14 rst_del1 (.Y(delay[00]), .A(~a)); //JLPL - 11_9_25
DLY10_X1N_A7P5PP84TR_C14 rst_del2 (.Y(delay[01]), .A(delay[00]));
DLY10_X1N_A7P5PP84TR_C14 rst_del3 (.Y(delay[02]), .A(delay[01]));
DLY10_X1N_A7P5PP84TR_C14 rst_del4 (.Y(delay[03]), .A(delay[02]));
DLY10_X1N_A7P5PP84TR_C14 rst_del5 (.Y(delay[04]), .A(delay[03]));
DLY10_X1N_A7P5PP84TR_C14 rst_del6 (.Y(delay[05]), .A(delay[04]));
DLY10_X1N_A7P5PP84TR_C14 rst_del7 (.Y(delay[06]), .A(delay[05]));
DLY10_X1N_A7P5PP84TR_C14 rst_del8 (.Y(delay[07]), .A(delay[06]));
DLY10_X1N_A7P5PP84TR_C14 rst_del9 (.Y(delay[08]), .A(delay[07]));
DLY10_X1N_A7P5PP84TR_C14 rst_del10 (.Y(delay[09]), .A(delay[08]));
DLY10_X1N_A7P5PP84TR_C14 rst_del11 (.Y(delay[10]), .A(delay[09]));
DLY10_X1N_A7P5PP84TR_C14 rst_del12 (.Y(delay[11]), .A(delay[10]));
DLY10_X1N_A7P5PP84TR_C14 rst_del13 (.Y(delay[12]), .A(delay[11]));
DLY10_X1N_A7P5PP84TR_C14 rst_del14 (.Y(delay[13]), .A(delay[12]));
DLY10_X1N_A7P5PP84TR_C14 rst_del15 (.Y(delay[14]), .A(delay[13]));
DLY10_X1N_A7P5PP84TR_C14 rst_del16 (.Y(delay[15]), .A(delay[14]));
`endif // PureRTL

assign #4.68   A[00]  =   delay[00],
               A[01]  =   delay[01] &  A[00],
               A[02]  =   delay[02] &  A[01],
               A[03]  =   delay[03] &  A[02],
               A[04]  =   delay[04] &  A[03],
               A[05]  =   delay[05] &  A[04],
               A[06]  =   delay[06] &  A[05],
               A[07]  =   delay[07] &  A[06],
               A[08]  =   delay[08] &  A[07],
               A[09]  =   delay[09] &  A[08],
               A[10]  =   delay[10] &  A[09],
               A[11]  =   delay[11] &  A[10],
               A[12]  =   delay[12] &  A[11],
               A[13]  =   delay[13] &  A[12],
               A[14]  =   delay[14] &  A[13],
               A[15]  =   delay[15] &  A[14];

assign   y =  ~A[15];

endmodule

/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

`ifdef   verify_persistence

module t;

reg   power_on_reset;
wire  delay1, delay2;

Persistence mut1 (
   .y(delay1),
   .a(power_on_reset)
   );

wire  reset_ring_oscillator   = ~power_on_reset & delay1;
wire  enable_ring_oscillator  = ~delay1;

Persistence mut2 (
   .y(delay2),
   .a(delay1)
   );

wire  system_reset   = delay2;

reg   start_ring_oscillator;
reg   alter_ring_oscillator;
always @(posedge RO or posedge reset_ring_oscillator)
   if       (reset_ring_oscillator) start_ring_oscillator <= #1 1'b1;
   else if  (alter_ring_oscillator) start_ring_oscillator <= #1 1'b0;


wire        RO;
A2_RingOscillator mut(
   .RO    (RO),
   .ctrl  ( start_ring_oscillator ? 3'b000 : TCC),
   .enable(enable_ring_oscillator)
   );

reg [2:0]   TCC;

initial begin
   power_on_reset = 1'bx;
   #1000;
   power_on_reset = 1'b1;
   #35000;
   power_on_reset = 1'b0;
   #10;
   power_on_reset = 1'b1;
   #30;
   power_on_reset = 1'b0;
   #10000;
   TCC   = 3'b111;
   @(posedge RO); #1;
   alter_ring_oscillator = 1'b1;
   @(posedge RO); #1;
   alter_ring_oscillator = 1'b0;
   #10000;
   power_on_reset = 1'b1;
   #10000;
   $stop;
   end

endmodule
`endif
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
