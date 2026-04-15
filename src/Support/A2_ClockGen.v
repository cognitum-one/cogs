/*==============================================================================================================================

    Copyright © 2021..2024 Advanced Architectures

    All rights reserved
    Confidential Information
    Limited Distribution to Authorized Persons Only
    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

    Project Name         : Library

    Description          : MODEL of a Controllable Ring Oscillator

================================================================================================================================
    THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
    BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
    TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
    AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
    ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
================================================================================================================================

PROTOTYPE:

A2_ClockGen i_CLK (
   // System Interconnect
   .iCG        (),
   .wCG        (),
   // Local Interconnect
   .rCG        (),
   .CGo        (),
   .CGk        (),
   // Global
   .ref_clock  (ref_clock),
   .xclock     (xclock),
   .xresetn    (xresetn)
   );

Control Register C [2:0]
   111:  ~25.25GHz                           /2    12.625
   110:  ~24.6 GHz                                 12.3
   101:  ~24.0 GHz                                 12.0
   100:  ~23.5 GHz                                 11.75
   011:  ~22.9 GHz                                 11.45
   010:  ~22.4 GHz                                 11.2
   001:  ~21.9 GHz                                 10.95
   000:  ~21.4 GHz - lowest frequency at reset     10.7

==============================================================================================================================*/

module A2_ClockGen (
   output wire             ref_clock,
   // System Interconnect
   input  wire [ 3:0]      iCG,
   input  wire             wCG,
   // Local Interconnect
   input  wire             rCG,
   output wire [ 3:0]       CGo,
   // Global
   input  wire             resetn,
   input  wire             clock
   );

// Control register -------------------------------------------------------------------------------------------------------------

// IMPERATIVE that 'c' set to zero first to ensure start up of Ring Oscillator

reg   [3:0] c;
always @(posedge clock or negedge resetn)
   if (!resetn)
      c[3:0]   <= 4'b0000;
   else if (wCG)
      c[3:0]   <= iCG[3:0];

// Oscillator ------------------------------------------------------------------------------------------------------------------
`ifdef   REAL_DEVICE
RingOsc_A9_C14 i_RO    (.clock(ref_clock), .d2(c[2]), .d1(c[1]), .d0(c[0]), .EN(resetn & c[3]));
`else
A2_RingOscillator i_RO (.RO(ref_clock),    .ctrl(c[2:0]),               .enable(resetn & c[3]));
`endif

// Outputs ---------------------------------------------------------------------------------------------------------------------

assign   CGo   =   rCG ? c[3:0] : 4'h0;

endmodule

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

`ifdef   verify_CG
`timescale  1ps/1fs

module t;

reg   [3:0] iCG;
reg         rCG;
reg         wCG;
reg         enable;
reg         xclock;
reg         xresetn;

wire  [3:0] CGo;
wire        CGk;
wire        ref_clock;
integer     i;

A2_ClockGen mut (
   // Internal Read -----------
   .CGo        ( CGo[3:0]),
   .rCG        (rCG),
   // System Interconnect -----
   .iCG        (iCG[3:0]),
   .wCG        (wCG),
   // Global ------------------
   .ref_clock  (ref_clock),
   .clock      (xclock),
   .reset      (~xresetn)
   );

initial begin
   xclock = 0;
   forever
      xclock = #100 ~xclock;
   end

initial begin
   iCG         =  4'h0;
   rCG         =  1'b0;
   wCG         =  1'b0;
   #100;
   enable      =  1'b0;
   xresetn     =  1'b1;
   #13;
   xresetn     =  1'b0;
   enable      =  1'b1;
   #500;
   xresetn     =  1'b1;
   #5000;
   rCG         =  1'b1;
   repeat (16)  @(posedge xclock) #1;
   for (i=0; i<8; i=i+1) begin
      iCG = 4'h8 + i;
      @(posedge xclock) #1;
      wCG         =  1'b1;
      @(posedge xclock) #1;
      wCG         =  1'b0;
      repeat (4)  @(posedge xclock) #1;
      end
   #500;
   iCG = 4'h8;
   @(posedge xclock) #1;
   wCG         =  1'b1;
   @(posedge xclock) #1;
   wCG         =  1'b0;
   #500;
   iCG = 4'hF;
   @(posedge xclock) #1;
   wCG         =  1'b1;
   @(posedge xclock) #1;
   wCG         =  1'b0;
   #500;

   enable      = 1'b0;
   #500;

   $stop;
   $finish;
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
