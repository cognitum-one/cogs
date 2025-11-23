/*==============================================================================================================================

   Copyright © 2003 Advanced Architectures

   All rights reserved
   Confidential Information
   Limited Distribution to Authorized Persons Only
   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

   Project Name         : "Common"
   Description          : Common Configuration file

   Author               : RTT
   Version              : 1.0
   Creation Date        : 5/27/2003

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

`ifndef  A2_DEFINES
`define  A2_DEFINES

// Timescale and Unit Delays ===================================================================================================

`ifndef  synthesis
// synopsys  translate_off
// synthesis translate_off

   `timescale  1ns/1ps

   `define  UNIT_DELAY  1

   `define  UD(a)   #(`UNIT_DELAY * a)
   `define  tCQ  `UD(1)
   `define  tUD  `UD(1)
   `define  tRA  `UD(15)
   `define  tWA  `UD(16)
   `define  tGQ  `UD(1)

// synthesis translate_on
// synopsys  translate_on

`else
   `define  UD(a)   #(a)
   `define  tCQ
   `define  tUD
   `define  tGQ
`endif

// For pulsed latches  => tPW is a parameter, hence no #
`define  tPW   1

// LOG FUNCTION ================================================================================================================

`define  LG2(x)   x  <=           1 ?  0     \
               :  x  <=           2 ?  1     \
               :  x  <=           4 ?  2     \
               :  x  <=           8 ?  3     \
               :  x  <=          16 ?  4     \
               :  x  <=          32 ?  5     \
               :  x  <=          64 ?  6     \
               :  x  <=         128 ?  7     \
               :  x  <=         256 ?  8     \
               :  x  <=         512 ?  9     \
               :  x  <=        1024 ? 10     \
               :  x  <=        2048 ? 11     \
               :  x  <=        4096 ? 12     \
               :  x  <=        8192 ? 13     \
               :  x  <=       16384 ? 14     \
               :  x  <=       32768 ? 15     \
               :  x  <=       65536 ? 16     \
               :  x  <=      131072 ? 17     \
               :  x  <=      262144 ? 18     \
               :  x  <=      524288 ? 19     \
               :  x  <=     1048576 ? 20     \
               :  x  <=     2097152 ? 21     \
               :  x  <=     4194304 ? 22     \
               :  x  <=     8388608 ? 23     \
               :  x  <=    16777216 ? 24     \
               :  x  <=    33554432 ? 25     \
               :  x  <=    67108864 ? 26     \
               :  x  <=   134217728 ? 27     \
               :  x  <=   268435456 ? 28     \
               :  x  <=   536870912 ? 29     \
               :  x  <=  1073741824 ? 30 : 31

// Set CLOCK configuration  -- Uncomment One -----------------------------------------------------------------------------------

`define     POSITIVE_EDGE_CLOCK
//`define     NEGATIVE_EDGE_CLOCK

// Set flip-flop reset mode -- Uncomment One -----------------------------------------------------------------------------------

`define     SYNC_HIGH_RESET
//`define     ASYNC_HIGH_RESET
//`define     SYNC_LOW_RESET
//`define     ASYNC_LOW_RESET


// Set Flip-flop configurations ================================================================================================
//
// All A2 IP cores use flip-flops specified in the same manner.  This block allows for the core to use rising or falling edge
// clocks and synchronous or asynchronous, positive or negative edge or level of reset.  The standard form is:
//
//    always @(`ACTIVE_EDGE clock `or_ACTIVE_EDGE_reset) begin
//       if (`ACTIVE_LEVEL_reset) begin
//          reset statement;
//          end
//       else begin
//          assignment statement;
//          end
//       end
//
// Selecting (by uncommenting) one of the options below any style of flip-flop style is supported by all A2 IP cores
//
// =============================================================================================================================
//

`ifdef NEGATIVE_EDGE_CLOCK
   `define ACTIVE_EDGE   negedge
   `define INACTIVE_EDGE posedge
   `define CLOCK_SENSE   ~
`else
   `define ACTIVE_EDGE   posedge
   `define INACTIVE_EDGE negedge
   `define CLOCK_SENSE
`endif

`ifdef SYNC_HIGH_RESET
   // Active High Synchronous Reset
   `define or_ACTIVE_EDGE(a)
   `define or_ACTIVE_EDGE_reset
   `define or_ACTIVE_EDGE_RESET
   `define ACTIVE_LEVEL
   `define ACTIVE_LEVEL_reset    reset
   `define ACTIVE_LEVEL_RESET    RESET
   `define reset_ON              1'b 1
   `define RESET_ON              1'b 1
   `define reset_OFF             1'b 0
   `define RESET_OFF             1'b 0
   `define RESET_HIGH
   `define RESET_SENSE
`else
   `ifdef ASYNC_HIGH_RESET
      // Positive Edge Active High Asynchronous Reset
      `define or_ACTIVE_EDGE(a)     or posedge a
      `define or_ACTIVE_EDGE_reset  or posedge reset
      `define or_ACTIVE_EDGE_RESET  or posedge RESET
      `define ACTIVE_LEVEL
      `define ACTIVE_LEVEL_reset    reset
      `define ACTIVE_LEVEL_RESET    RESET
      `define reset_ON              1'b 1
      `define RESET_ON              1'b 1
      `define reset_OFF             1'b 0
      `define RESET_OFF             1'b 0
      `define RESET_HIGH
      `define RESET_SENSE
   `else
      `ifdef SYNC_LOW_RESET
         // Active Low  Synchronous Reset
         `define or_ACTIVE_EDGE(a)
         `define or_ACTIVE_EDGE_reset
         `define or_ACTIVE_EDGE_RESET
         `define ACTIVE_LEVEL          !
         `define ACTIVE_LEVEL_reset    !reset
         `define ACTIVE_LEVEL_RESET    !RESET
         `define reset_ON              1'b 0
         `define RESET_ON              1'b 0
         `define reset_OFF             1'b 1
         `define RESET_OFF             1'b 1
         `define RESET_LOW
         `define RESET_SENSE           ~
      `else
          // Negative Edge Active Low  Asynchronous Reset
         `define or_ACTIVE_EDGE(a)     or negedge a
         `define or_ACTIVE_EDGE_reset  or negedge reset
         `define or_ACTIVE_EDGE_RESET  or negedge RESET
         `define ACTIVE_LEVEL          !
         `define ACTIVE_LEVEL_reset    !reset
         `define ACTIVE_LEVEL_RESET    !RESET
         `define reset_ON              1'b 0
         `define RESET_ON              1'b 0
         `define reset_OFF             1'b 1
         `define RESET_OFF             1'b 1
         `define RESET_LOW
         `define RESET_SENSE           ~
      `endif
   `endif
`endif

`endif
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
