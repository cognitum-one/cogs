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

// `ifndef  PROJECT_SETTINGS
// `ifndef  synthesis
// // synopsys  translate_off
// // synthesis translate_off
//
//    `timescale  1ns/1ps
//
//    `ifndef  tCQ
//    `define  tCQ   #0.01
//    `endif //tCQ
//
//    `ifndef  tACC
//    `define  tACC  #0.10
//    `endif //tACC
//
// // synthesis translate_on
// // synopsys  translate_on
// `endif //synthesis
// `endif //PROJECT_SETTINGS

// LOG FUNCTION ================================================================================================================

`define  LG2(x)  x <=         1 ?  0  : x <=         2 ?  1  : x <=          4 ?  2  : x <=         8 ?  3  : x <=        16 ?  4  : x <=        32 ?  5  : x <=         64 ?  6  : x <=       128 ?  7  : x <=       256 ?  8  : x <=       512 ?  9  : x <=       1024 ? 10  : x <=      2048 ? 11  : x <=      4096 ? 12  : x <=      8192 ? 13  : x <=      16384 ? 14  : x <=     32768 ? 15  : x <=     65536 ? 16  : x <=    131072 ? 17  : x <=     262144 ? 18  : x <=    524288 ? 19  : x <=   1048576 ? 20  : x <=   2097152 ? 21  : x <=    4194304 ? 22  : x <=   8388608 ? 23  : x <=  16777216 ? 24  : x <=  33554432 ? 25  : x <=   67108864 ? 26  : x <= 134217728 ? 27  : x <= 268435456 ? 28  : x <= 536870912 ? 29  : x <= 1073741824 ? 30  : 31

`endif //A2_DEFINES

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
