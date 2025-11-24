//==============================================================================================================================
//
//   Copyright © 1996..2022 Advanced Architectures
//
//   All rights reserved
//   Confidential Information
//   Limited Distribution to authorized Persons Only
//   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//   Project Name         : Clock Domain Crossing Interface
//
//===============================================================================================================================
//      THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//===============================================================================================================================
//
//   Closed loop synchronizers are used.  The write signal is turned into a level transition and passed through a synchronizer
//   into the output clock domain.  It is then returned as an acknowledge back through a synchronizer to the input clock domain.
//   It is also turned back into a pulse for use in the read clock domain.
//
//   The CDCld signal is used to load, on the input clock, an external register with the bundled data that must cross the clock
//   domain boundary.  The register output is guaranteed stable in the output clock domain by the signal CDCor (output ready)
//   which is synchonized to the output clock.
//
//   The output side must consume the output data (CDCo) by asserting read the data (rCDC) after the output ready (CDCor) is
//   valid.  This initiates the acknowledge back to the input side to complete the closed-loop signaling.
//
//   This is not meant to be a high speed interface passing lots of data as several clock cyles are required for each
//   transaction and depends on the relative clock rates of the input and output sides.  Using a larger width for the data can
//   offset this to some extent.
//
//   Standard FIFO-like signaling is used so direct connection on either side to a synchronous FIFO is possible.
//
//   The A2_sync modules should be replaced with standard cell library modules whenever possible
//
//===============================================================================================================================
//   Prototype:
//
//   A2_CDC inst (
//      // Input side
//       .CDCir   (),      // Input ready
//       .CDCld   (),      // Load data bundle register
//      .wCDC     (),      // Write input data
//      .ireset   (),      // reset synchronized to iclock
//      .iclock   (),      // input  clock
//      // Output Side
//       .CDCor   (),      // Output ready
//      .rCDC     (),      // Read output data
//      .oreset   (),      // reset synchronized to oclock
//      .oclock   ()       // output clock
//      );
//
//===============================================================================================================================

`ifdef   RELATIVE_FILENAMES
//`include "A2_project_settings.vh"
`else
//`include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/A2_project_settings.vh"
`endif

module A2_CDC (
   // Input side
   output wire              CDCir,     // Input ready
   output wire              CDCld,     // Load data bundle register
   input  wire             wCDC,       // Write input data
   input  wire             ireset,     // reset synchronized to iclock
   input  wire             iclock,     // input  clock
   // Output Side
   output wire              CDCor,     // Output ready
   input  wire             rCDC,       // Read output data
   input  wire             oreset,     // reset synchronized to oclock
   input  wire             oclock      // output clock
   );

// Locals
reg   Req,  Ack;
wire  sack, sreq, resp;

// Input Side
assign   CDCld =  CDCir &  wCDC;

always @(posedge iclock)
   if      (ireset) Req <= `tCQ  1'b0;
   else if (CDCld ) Req <= `tCQ ~Req;  // Convert pulse into a level so we don't lose it regardless of clock widths!

A2_sync  i_SA  (.q(sack),.d(Ack),.reset(ireset),.clock(iclock));

assign   CDCir =  Req  ~^ sack;

// Output Side
assign   resp  =  CDCor &  rCDC;

always @(posedge oclock or posedge oreset)
   if      (oreset) Ack <= `tCQ  1'b0;
   else if (resp  ) Ack <= `tCQ ~Ack;  // Convert pulse into a level so we don't lose it regardless of clock widths!

A2_sync  i_SR  (.q(sreq),.d(Req),.reset(oreset),.clock(oclock));

assign   CDCor =  Ack   ^ sreq;


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
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

`ifdef   verify_CDC
// Logical verification test only!!!  Needs to be evaluated for asynchronus violations
`timescale 1ps/1fs

module t;

parameter   WIDTH = 32;
parameter   ITICK = 7;
parameter   OTICK = 23;

reg  [WIDTH-1:0]  CDCo;      // Data out of parameterized width
reg  [WIDTH-1:0] iCDC;       // input data of parameterized width

wire              CDCor;     // Output ready
wire              CDCir;     // Input ready
wire              CDCld;     // Load data bundle to transfer
reg              wCDC;       // Write input data
reg              rCDC;       // Read output data
wire             iresetn;    // reset synchronized to iclock
wire             oresetn;    // reset synchronized to oclock
reg              aresetn;    // asynchronous reset
reg              iclock;     // input  clock
reg              oclock;     // output clock

A2_sync  isi  (.q(iresetn),.d(aresetn),.resetn(aresetn),.clock(iclock));
A2_sync  iso  (.q(oresetn),.d(aresetn),.resetn(aresetn),.clock(oclock));

A2_CDC mut (
   .CDCor   (CDCor),      // Output ready
   .CDCir   (CDCir),      // Input ready
   .CDCld   (CDCld),      // Load
   .wCDC    (wCDC),       // Write input data
   .rCDC    (rCDC),       // Read output data
   .iresetn (iresetn),    // reset synchronized to iclock
   .oresetn (oresetn),    // reset synchronized to oclock
   .iclock  (iclock),     // input  clock
   .oclock  (oclock)      // output clock
   );

always @(posedge iclock)
   if (CDCld) CDCo[WIDTH-1:0] <= #1 iCDC[WIDTH-1:0];

initial begin
   iclock     = 0;
   forever begin
      #(ITICK/2); iclock = ~iclock;
      end
   end

initial begin
   oclock     = 0;
   forever begin
      #(OTICK/2); oclock = ~oclock;
      end
   end

initial begin
   aresetn  = 1;
   iCDC     = 0;
   wCDC     = 0;
   rCDC     = 0;
   #100;
   aresetn  = 0;
   #1000;
   aresetn  = 1;
   #100
   fork
      begin : send
      @(posedge iclock); #1;
      wCDC = 1;
      iCDC = 32'h600df1d0;
      @(posedge iclock); #1;
      while (!CDCir) @(posedge iclock); #1;
      iCDC = 32'hdeadbeef;
      @(posedge iclock); #1;
      while (!CDCir) @(posedge iclock); #1;
      wCDC = 0;
      @(posedge iclock); #1;
      end

      begin : receive
      rCDC = 1;
      while (!CDCor) @(posedge oclock); #1;
      @(posedge oclock); #1;
      rCDC = 1;
      while (!CDCor) @(posedge oclock); #1;
      end
   join
   #1000;
   $finish;
   $stop;
   end

endmodule
`endif
